pub struct Graph {
    ptr: i32,
}
impl Graph {
    pub fn add<'a, 'b>(&'a mut self) -> Context<'b>
    where
        'a: 'b,
    {
        Context { _marker: "123" }
    }
}
pub struct Context<'a> {
    _marker: &'a str,
}
#[derive(Default)]
pub struct FilterCtx<'a> {
    buffersink_ctx: Option<Context<'a>>, // AVFilterContext
    buffersrc_ctx: Option<Context<'a>>,
    filter_graph: Option<Graph>,
}
impl<'a> FilterCtx<'a> {
    fn init_filter(&mut self) {
        let mut filter_graph = Graph { ptr: 1 };
        let ctx1 = filter_graph.add(); // 报错 `filter_graph` does not live long enough borrowed value does not live long enough
        let ctx2 = filter_graph.add();
        self.buffersink_ctx = Some(ctx1);
        self.buffersrc_ctx = Some(ctx2);
    }
}
fn main() {}
