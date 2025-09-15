use super::super::core;
use deno_core::{serde, serde_v8, v8};

// TODO: addShape, addText の This は Extern ではなく Value にする
pub(crate) enum Shape {
    Shape {
        shape: Vec<(f64, f64)>, // [[x1, y1], [x2, y2], ...]
        fill: Option<u32>,
        stroke: Option<u32>,
        stroke_width: Option<f64>,
        stroke_closed: Option<bool>,
    },
    Text {
        text: String,
        x: f64,
        y: f64,
        size: Option<f64>,
        color: Option<u32>,
    },
}
pub(crate) struct Shapes(Vec<Shape>);
impl Shapes {
    pub(crate) fn new() -> Self {
        Self(Vec::new())
    }
    pub(crate) fn add_shape(&mut self, info: core::CallbackInfo) {
        /*
            addShape: (shape: [number, number][], options?: {
              fill?: number,
              stroke?: number,
              strokeWidth?: number,
              strokeClosed?: boolean,
            }) => void,
        */
        type Arg0 = Vec<(f64, f64)>;
        #[derive(serde::Deserialize)]
        struct Arg1 {
            fill: Option<u32>,
            stroke: Option<u32>,
            stroke_width: Option<f64>,
            stroke_closed: Option<bool>,
        }
        let Ok(shape) = serde_v8::from_v8::<Arg0>(info.scope, info.args.get(0)) else {
            let msg = v8::String::new(info.scope, "argument 0 is invalid")
                .unwrap_or(v8::String::empty(info.scope));
            let err = v8::Exception::type_error(info.scope, msg);
            info.scope.throw_exception(err);
            return;
        };
        let Ok(options) = serde_v8::from_v8::<Option<Arg1>>(info.scope, info.args.get(1)) else {
            let msg = v8::String::new(info.scope, "argument 1 is invalid")
                .unwrap_or(v8::String::empty(info.scope));
            let err = v8::Exception::type_error(info.scope, msg);
            info.scope.throw_exception(err);
            return;
        };
        if let Some(options) = options.as_ref() {
            self.0.push(Shape::Shape {
                shape,
                fill: options.fill,
                stroke: options.stroke,
                stroke_width: options.stroke_width,
                stroke_closed: options.stroke_closed,
            });
        } else {
            self.0.push(Shape::Shape {
                shape,
                fill: None,
                stroke: None,
                stroke_width: None,
                stroke_closed: None,
            });
        }
    }
    pub(crate) fn add_text(&mut self, info: core::CallbackInfo) {
        /*
            addText: (text: string, x: number, y: number, options?: {
              size?: number,
              color?: number,
            }) => void,
        */
        #[derive(serde::Deserialize)]
        struct Arg2 {
            fill: Option<u32>,
            stroke: Option<u32>,
            stroke_width: Option<f64>,
            stroke_closed: Option<bool>,
        }
        let Ok(text) = serde_v8::from_v8::<String>(info.scope, info.args.get(0)) else {
            let msg = v8::String::new(info.scope, "argument 0 must be a string")
                .unwrap_or(v8::String::empty(info.scope));
            let err = v8::Exception::type_error(info.scope, msg);
            info.scope.throw_exception(err);
            return;
        };
        let Ok(x) = serde_v8::from_v8::<f64>(info.scope, info.args.get(1)) else {
            let msg = v8::String::new(info.scope, "argument 1 must be a number")
                .unwrap_or(v8::String::empty(info.scope));
            let err = v8::Exception::type_error(info.scope, msg);
            info.scope.throw_exception(err);
            return;
        };
        let Ok(y) = serde_v8::from_v8::<f64>(info.scope, info.args.get(2)) else {
            let msg = v8::String::new(info.scope, "argument 2 must be a number")
                .unwrap_or(v8::String::empty(info.scope));
            let err = v8::Exception::type_error(info.scope, msg);
            info.scope.throw_exception(err);
            return;
        };
        let Ok(options) = serde_v8::from_v8::<Option<Arg2>>(info.scope, info.args.get(2)) else {
            let msg = v8::String::new(info.scope, "argument 3 is invalid")
                .unwrap_or(v8::String::empty(info.scope));
            let err = v8::Exception::type_error(info.scope, msg);
            info.scope.throw_exception(err);
            return;
        };
        if let Some(options) = options.as_ref() {
            self.0.push(Shape::Text {
                text,
                x,
                y,
                size: options.stroke_width,
                color: options.stroke,
            });
        } else {
            self.0.push(Shape::Text {
                text,
                x,
                y,
                size: None,
                color: None,
            });
        }
    }
}
