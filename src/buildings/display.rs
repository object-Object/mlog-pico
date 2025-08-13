use core::fmt::Debug;

use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::*,
    primitives::{
        Line, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, StyledDrawable, Triangle,
    },
};
use mindustry_rs::{
    types::LAccess,
    vm::{CustomBuildingData, DrawCommand, InstructionResult, LValue, LogicVM, ProcessorState},
};

pub struct DisplayData<T, S> {
    display: T,
    line_style: S,
    fill_style: S,
    translation: Point,
    operations: usize,
}

impl DisplayData<(), ()> {
    // https://github.com/Anuken/Mindustry/blob/65a50a97423431640e636463dde97f6f88a2b0c8/core/src/mindustry/graphics/Pal.java#L35
    pub const RESET_COLOR: Rgb888 = Rgb888::new(0x56, 0x56, 0x66);
}

impl<T> DisplayData<T, PrimitiveStyle<T::Color>>
where
    T: DrawTarget,
    T::Color: From<Rgb888>,
{
    pub fn new(display: T) -> Self {
        let style = PrimitiveStyleBuilder::new()
            .stroke_color(Rgb888::WHITE.into())
            .stroke_width(1);

        Self {
            display,
            line_style: style.build(),
            fill_style: style.fill_color(Rgb888::WHITE.into()).build(),
            translation: Point::zero(),
            operations: 0,
        }
    }

    fn draw(
        &mut self,
        drawable: impl Primitive
        + StyledDrawable<PrimitiveStyle<T::Color>, Color = T::Color, Output = ()>
        + Transform,
        fill: bool,
    ) -> Result<(), T::Error> {
        if fill {
            self.draw_fill(drawable)
        } else {
            self.draw_line(drawable)
        }
    }

    fn draw_line(
        &mut self,
        drawable: impl StyledDrawable<PrimitiveStyle<T::Color>, Color = T::Color, Output = ()>
        + Transform,
    ) -> Result<(), T::Error> {
        drawable
            .translate(self.translation)
            .draw_styled(&self.line_style, &mut self.display)
    }

    fn draw_fill(
        &mut self,
        drawable: impl StyledDrawable<PrimitiveStyle<T::Color>, Color = T::Color, Output = ()>
        + Transform,
    ) -> Result<(), T::Error> {
        drawable
            .translate(self.translation)
            .draw_styled(&self.fill_style, &mut self.display)
    }

    fn draw_command(&mut self, command: &DrawCommand) -> Result<(), T::Error> {
        match command {
            &DrawCommand::Clear { r, g, b } => self.display.clear(Rgb888::new(r, g, b).into()),

            &DrawCommand::Color { r, g, b, a } => {
                let color = if a > 0 {
                    Some(Rgb888::new(r, g, b).into())
                } else {
                    None
                };
                self.line_style.stroke_color = color;
                self.fill_style.stroke_color = color;
                self.fill_style.fill_color = color;
                Ok(())
            }

            &DrawCommand::Stroke { width } => {
                self.line_style.stroke_width = width as u32;
                self.fill_style.stroke_width = width as u32;
                Ok(())
            }

            &DrawCommand::Line { x1, y1, x2, y2 } => self.draw_line(Line::new(
                Point::new(x1 as i32, y1 as i32),
                Point::new(x2 as i32, y2 as i32),
            )),

            &DrawCommand::Rect {
                x,
                y,
                width,
                height,
                fill,
            } => self.draw(
                Rectangle::new(
                    Point::new(x as i32, y as i32),
                    Size::new(width as u32, height as u32),
                ),
                fill,
            ),

            // TODO: implement
            // fill: https://github.com/embedded-graphics/embedded-graphics/issues/293
            &DrawCommand::Poly { .. } => Ok(()),

            &DrawCommand::Triangle {
                x1,
                y1,
                x2,
                y2,
                x3,
                y3,
            } => self.draw_fill(Triangle::new(
                Point::new(x1 as i32, y1 as i32),
                Point::new(x2 as i32, y2 as i32),
                Point::new(x3 as i32, y3 as i32),
            )),

            // TODO: implement
            &DrawCommand::Image { .. } => Ok(()),

            // TODO: implement
            DrawCommand::Print { .. } => Ok(()),

            &DrawCommand::Translate { x, y } => {
                self.translation = Point::new(x as i32, y as i32);
                Ok(())
            }

            // TODO: implement
            DrawCommand::Scale { .. } | DrawCommand::Rotate { .. } => Ok(()),

            DrawCommand::Reset => {
                self.translation = Point::zero();
                Ok(())
            }
        }
    }
}

impl<T> CustomBuildingData for DisplayData<T, PrimitiveStyle<T::Color>>
where
    T: DrawTarget,
    T::Color: From<Rgb888>,
    T::Error: Debug,
{
    fn drawflush(&mut self, state: &mut ProcessorState, _: &LogicVM) -> InstructionResult {
        for command in &state.drawbuffer {
            self.draw_command(command).unwrap();
        }

        self.operations += 1;

        // we just did a lot of blocking calls, so yield to let other threads run
        // TODO: use async instead?
        InstructionResult::Yield
    }

    fn sensor(&mut self, _: &mut ProcessorState, _: &LogicVM, sensor: LAccess) -> Option<LValue> {
        Some(match sensor {
            LAccess::DisplayWidth => self.display.bounding_box().size.width.into(),
            LAccess::DisplayHeight => self.display.bounding_box().size.height.into(),
            LAccess::Operations => self.operations.into(),
            _ => return None,
        })
    }
}
