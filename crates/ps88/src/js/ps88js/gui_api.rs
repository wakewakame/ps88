use super::super::core;
use deno_core::serde::{Deserialize, Serialize};
use deno_core::{serde_v8, v8};

type Color = u32; // 0xRRGGBBAA

#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub enum Shape {
    Polygon {
        path: Vec<(f64, f64)>, // [[x1, y1], [x2, y2], ...]
        fill: Option<Color>,
        stroke: Option<Color>,
        stroke_width: Option<f64>,
        stroke_closed: Option<bool>,
    },
    Text {
        text: String,
        x: f64,
        y: f64,
        size: Option<f64>,
        color: Option<Color>,
    },
}

pub(super) fn gen_api_add_polygon<'b>(
    scope: &mut v8::HandleScope<'b>,
    shapes: v8::Local<'b, v8::Array>,
) -> core::Result<v8::Local<'b, v8::Function>> {
    fn callback(
        scope: &mut v8::HandleScope,
        args: v8::FunctionCallbackArguments,
        _: v8::ReturnValue,
    ) {
        type Arg0 = Vec<(f64, f64)>;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Arg1 {
            fill: Option<Color>,
            stroke: Option<Color>,
            stroke_width: Option<f64>,
            stroke_closed: Option<bool>,
        }
        let path = match serde_v8::from_v8::<Arg0>(scope, args.get(0)) {
            Ok(path) => path,
            Err(err) => {
                return core::v8throw_type_error(scope, &format!("arg0: {}", err));
            }
        };
        let options = match serde_v8::from_v8::<Option<Arg1>>(scope, args.get(1)) {
            Ok(options) => options,
            Err(err) => {
                return core::v8throw_type_error(scope, &format!("arg1: {}", err));
            }
        };
        let shapes = match args.data().try_cast::<v8::Array>() {
            Ok(v) => v,
            Err(err) => {
                return core::v8throw(
                    scope,
                    &format!("unexpected: \"this\" is not an array: {}", err),
                );
            }
        };
        let shape = Shape::Polygon {
            path,
            fill: options.as_ref().and_then(|o| o.fill),
            stroke: options.as_ref().and_then(|o| o.stroke),
            stroke_width: options.as_ref().and_then(|o| o.stroke_width),
            stroke_closed: options.as_ref().and_then(|o| o.stroke_closed),
        };
        let shape_js = match serde_v8::to_v8(scope, &shape) {
            Ok(v) => v,
            Err(err) => {
                return core::v8throw(scope, &format!("unexpected: {}", err));
            }
        };
        shapes.set_index(scope, shapes.length(), shape_js);
    }
    v8::FunctionBuilder::<v8::Function>::new(callback)
        .data(shapes.into())
        .build(scope)
        .ok_or(core::JsRuntimeError::Runtime(
            "failed to create add_polygon function".to_string(),
        ))
}

pub(super) fn gen_api_add_text<'b>(
    scope: &mut v8::HandleScope<'b>,
    shapes: v8::Local<'b, v8::Array>,
) -> core::Result<v8::Local<'b, v8::Function>> {
    fn callback(
        scope: &mut v8::HandleScope,
        args: v8::FunctionCallbackArguments,
        _: v8::ReturnValue,
    ) {
        type Arg0 = String;
        type Arg1 = f64;
        type Arg2 = f64;
        #[derive(Deserialize)]
        struct Arg3 {
            size: Option<f64>,
            color: Option<Color>,
        }
        let text = match serde_v8::from_v8::<Arg0>(scope, args.get(0)) {
            Ok(path) => path,
            Err(err) => {
                return core::v8throw_type_error(scope, &format!("arg0: {}", err));
            }
        };
        let x = match serde_v8::from_v8::<Arg1>(scope, args.get(1)) {
            Ok(options) => options,
            Err(err) => {
                return core::v8throw_type_error(scope, &format!("arg1: {}", err));
            }
        };
        let y = match serde_v8::from_v8::<Arg2>(scope, args.get(2)) {
            Ok(options) => options,
            Err(err) => {
                return core::v8throw_type_error(scope, &format!("arg2: {}", err));
            }
        };
        let options = match serde_v8::from_v8::<Option<Arg3>>(scope, args.get(3)) {
            Ok(options) => options,
            Err(err) => {
                return core::v8throw_type_error(scope, &format!("arg3: {}", err));
            }
        };
        let shapes = match args.data().try_cast::<v8::Array>() {
            Ok(v) => v,
            Err(err) => {
                return core::v8throw(
                    scope,
                    &format!("unexpected: \"this\" is not an array: {}", err),
                );
            }
        };
        let shape = Shape::Text {
            text,
            x,
            y,
            size: options.as_ref().and_then(|o| o.size),
            color: options.as_ref().and_then(|o| o.color),
        };
        let shape_js = match serde_v8::to_v8(scope, &shape) {
            Ok(v) => v,
            Err(err) => {
                return core::v8throw(scope, &format!("unexpected: {}", err));
            }
        };
        shapes.set_index(scope, shapes.length(), shape_js);
    }
    v8::FunctionBuilder::<v8::Function>::new(callback)
        .data(shapes.into())
        .build(scope)
        .ok_or(core::JsRuntimeError::Runtime(
            "failed to create add_text function".to_string(),
        ))
}
