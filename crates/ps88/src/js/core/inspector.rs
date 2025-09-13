use deno_core::v8;

pub struct Inspector<'a> {
    // NOTE: _inspector は _client の参照を抱えるため client より先に drop される必要がある
    _inspector: v8::UniqueRef<v8::inspector::V8Inspector>,
    _client: Box<InspectorClient<'a>>,
}

impl<'a> Inspector<'a> {
    pub fn new<F: Fn(String) + 'a>(
        scope: &mut v8::HandleScope,
        context: v8::Local<v8::Context>,
        logger: F,
    ) -> Self {
        let mut client = {
            let base = v8::inspector::V8InspectorClientBase::new::<InspectorClient>();
            let logger = Box::new(logger);
            Box::new(InspectorClient { base, logger })
        };
        let inspector = {
            let mut inspector = v8::inspector::V8Inspector::create(scope, &mut *client);
            let context_name = v8::inspector::StringView::from(&b"main realm"[..]);
            let aux_data = v8::inspector::StringView::from(r#"{"isDefault": true}"#.as_bytes());
            inspector.context_created(context, 1, context_name, aux_data);
            inspector
        };
        Self {
            _inspector: inspector,
            _client: client,
        }
    }
}

struct InspectorClient<'a> {
    base: v8::inspector::V8InspectorClientBase,
    logger: Box<dyn Fn(String) + 'a>,
}

// 参考: https://github.com/denoland/deno_core/blob/75759fb5127982bdaf71e68f04dee01531d6591b/core/inspector.rs#L119
impl<'a> v8::inspector::V8InspectorClientImpl for InspectorClient<'a> {
    fn base(&self) -> &v8::inspector::V8InspectorClientBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut v8::inspector::V8InspectorClientBase {
        &mut self.base
    }

    unsafe fn base_ptr(this: *const Self) -> *const v8::inspector::V8InspectorClientBase
    where
        Self: Sized,
    {
        // SAFETY: this pointer is valid for the whole lifetime of inspector
        unsafe { std::ptr::addr_of!((*this).base) }
    }

    fn console_api_message(
        &mut self,
        _context_group_id: i32,
        _level: i32,
        message: &v8::inspector::StringView,
        _url: &v8::inspector::StringView,
        _line_number: u32,
        _column_number: u32,
        _stack_trace: &mut v8::inspector::V8StackTrace,
    ) {
        // ログメッセージの出力
        (self.logger)(message.to_string());
    }
}
