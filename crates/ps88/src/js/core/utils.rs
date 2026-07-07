use super::error::*;
use deno_core::v8;

pub fn v8str<'s>(
    scope: &mut v8::HandleScope<'s, ()>,
    s: &str,
) -> Result<v8::Local<'s, v8::String>> {
    let name = v8::String::new(scope, s).ok_or(JsRuntimeError::UnexpectedError(format!(
        "failed to create string: {}",
        s
    )))?;
    Ok(name)
}

pub fn v8throw<'s>(scope: &mut v8::HandleScope<'s>, s: &str) {
    let msg = v8::String::new(scope, s).unwrap_or(v8::String::empty(scope));
    let err = v8::Exception::error(scope, msg);
    scope.throw_exception(err);
}

pub fn v8throw_type_error<'s>(scope: &mut v8::HandleScope<'s>, s: &str) {
    let msg = v8::String::new(scope, s).unwrap_or(v8::String::empty(scope));
    let err = v8::Exception::type_error(scope, msg);
    scope.throw_exception(err);
}

// TryCatch からエラー情報を文字列に変換する
pub fn report_exceptions(try_catch: &mut v8::TryCatch<v8::HandleScope>) -> String {
    let mut description = Vec::<String>::new();
    if try_catch.has_terminated() {
        return "execution was terminated (script took too long)".into();
    }
    let Some(exception) = try_catch.exception() else {
        return "no error".into();
    };
    let Some(exception_string) = exception.to_string(try_catch) else {
        return "unexpected error".into();
    };
    let exception_string = exception_string.to_rust_string_lossy(try_catch);
    let Some(message) = try_catch.message() else {
        return exception_string;
    };

    // 該当箇所の出力
    // e.g.
    //   main.js:5: SyntaxError: Unexpected token '=='
    let filename = message
        .get_script_resource_name(try_catch)
        .and_then(|s| s.to_string(try_catch))
        .map(|s| s.to_rust_string_lossy(try_catch))
        .unwrap_or("(unknown)".into());
    let line_number = message
        .get_line_number(try_catch)
        .map(|n| n.to_string())
        .unwrap_or("(unknown)".into());
    description.push(format!(
        "{}:{}: {}",
        filename, line_number, exception_string
    ));

    // 該当箇所のコードを出力
    // e.g.
    //   let a == 1;
    //         ^^
    if let Some(source_line) = message.get_source_line(try_catch) {
        let source_line = source_line.to_rust_string_lossy(try_catch);
        let start_column = message.get_start_column();
        let end_column = message.get_end_column();
        description.push(format!(
            "\n{}\n{}{}\n",
            source_line,
            " ".repeat(start_column),
            "^".repeat(end_column - start_column)
        ));
    }

    // スタックトレースを出力
    // e.g.
    //   Error: aaa
    //       at f3 (<anonymous>:4:26)
    //       at f2 (<anonymous>:3:20)
    //       at f1 (<anonymous>:2:20)
    //       at main (<anonymous>:1:22)
    //       at <anonymous>:5:1
    if let Some(stack_trace) = try_catch
        .stack_trace()
        .and_then(|s| s.to_string(try_catch))
        .map(|s| s.to_rust_string_lossy(try_catch))
    {
        description.push(format!("{}", stack_trace));
    }

    return description.join("\n");
}
