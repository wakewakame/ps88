use super::super::core;
use deno_core::serde::de::DeserializeOwned;
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

// index 番目の引数を serde でデコードする。
// 失敗した場合は JavaScript 側に TypeError を投げて None を返す。
fn decode_arg<T: DeserializeOwned>(
    scope: &mut v8::HandleScope,
    args: &v8::FunctionCallbackArguments,
    index: i32,
) -> Option<T> {
    match serde_v8::from_v8::<T>(scope, args.get(index)) {
        Ok(v) => Some(v),
        Err(err) => {
            core::v8throw_type_error(scope, &format!("arg{}: {}", index, err));
            None
        }
    }
}

// コールバックの data() に紐付けた配列に shape を追加する
fn push_shape(scope: &mut v8::HandleScope, args: &v8::FunctionCallbackArguments, shape: Shape) {
    let shapes = match args.data().try_cast::<v8::Array>() {
        Ok(v) => v,
        Err(err) => {
            return core::v8throw(
                scope,
                &format!("unexpected: \"this\" is not an array: {}", err),
            );
        }
    };
    let shape_js = match serde_v8::to_v8(scope, &shape) {
        Ok(v) => v,
        Err(err) => {
            return core::v8throw(scope, &format!("unexpected: {}", err));
        }
    };
    shapes.set_index(scope, shapes.length(), shape_js);
}

// callback を shapes 配列に紐付けた v8::Function に変換する
fn build_shape_fn<'b>(
    scope: &mut v8::HandleScope<'b>,
    shapes: v8::Local<'b, v8::Array>,
    callback: impl v8::MapFnTo<v8::FunctionCallback>,
    name: &str,
) -> core::Result<v8::Local<'b, v8::Function>> {
    v8::FunctionBuilder::<v8::Function>::new(callback)
        .data(shapes.into())
        .build(scope)
        .ok_or(core::JsRuntimeError::RuntimeError(format!(
            "failed to create {} function",
            name
        )))
}

pub(super) fn gen_api_add_polygon<'a, 'b>(
    scope: &'a mut v8::HandleScope<'b>,
    shapes: v8::Local<'b, v8::Array>,
) -> core::Result<v8::Local<'b, v8::Function>> {
    fn callback(
        scope: &mut v8::HandleScope,
        args: v8::FunctionCallbackArguments,
        _: v8::ReturnValue,
    ) {
        #[derive(Deserialize, Default)]
        #[serde(rename_all = "camelCase")]
        struct Options {
            fill: Option<Color>,
            stroke: Option<Color>,
            stroke_width: Option<f64>,
            stroke_closed: Option<bool>,
        }
        let Some(path) = decode_arg::<Vec<(f64, f64)>>(scope, &args, 0) else {
            return;
        };
        let Some(options) = decode_arg::<Option<Options>>(scope, &args, 1) else {
            return;
        };
        let options = options.unwrap_or_default();
        push_shape(
            scope,
            &args,
            Shape::Polygon {
                path,
                fill: options.fill,
                stroke: options.stroke,
                stroke_width: options.stroke_width,
                stroke_closed: options.stroke_closed,
            },
        );
    }
    build_shape_fn(scope, shapes, callback, "add_polygon")
}

pub(super) fn gen_api_add_text<'a, 'b>(
    scope: &'a mut v8::HandleScope<'b>,
    shapes: v8::Local<'b, v8::Array>,
) -> core::Result<v8::Local<'b, v8::Function>> {
    fn callback(
        scope: &mut v8::HandleScope,
        args: v8::FunctionCallbackArguments,
        _: v8::ReturnValue,
    ) {
        #[derive(Deserialize, Default)]
        struct Options {
            size: Option<f64>,
            color: Option<Color>,
        }
        let Some(text) = decode_arg::<String>(scope, &args, 0) else {
            return;
        };
        let Some(x) = decode_arg::<f64>(scope, &args, 1) else {
            return;
        };
        let Some(y) = decode_arg::<f64>(scope, &args, 2) else {
            return;
        };
        let Some(options) = decode_arg::<Option<Options>>(scope, &args, 3) else {
            return;
        };
        let options = options.unwrap_or_default();
        push_shape(
            scope,
            &args,
            Shape::Text {
                text,
                x,
                y,
                size: options.size,
                color: options.color,
            },
        );
    }
    build_shape_fn(scope, shapes, callback, "add_text")
}
