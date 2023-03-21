use crate::ffmpeg::FFmpeg;
use ffmpeg_next::filter::{self, Filter};

pub trait OSD {
    fn init_filter(&mut self) {}
}
impl<'a> OSD for FFmpeg<'a> {
    fn init_filter(&mut self) {
        // filter register
        filter::register_all();

        // init gragh
        let mut filter_gh = filter::Graph::new();

        // buffer filter
        let buffer = filter::find("buffer").expect("Failed to create buffer filter");
        let buffer_ctx = filter_gh
            .add(&buffer, "in", "")
            .expect("Failed to add buffer filter to gh"); // TODO: args?

        // buffersink filter
        let buffer_sink = filter::find("buffersink").expect("Failed to create buffer_sink filter");
        let buffer_sink_ctx = filter_gh
            .add(&buffer_sink, "out", "")
            .expect("Failed to add buffer_sink filter to gh");

        // create parser
        let in_out = filter::graph::Parser::new(&mut filter_gh);
        let in_out = in_out.input("out", 0).unwrap();
        let in_out = in_out.output("in", 0).unwrap();

        // drawtext
        let drawtext = "drawtext=fontcolor=red:fontsize=50:x=0:y=0:text=";
        let date = "%{localtime\\:%a %b %d %Y}";
        let drawtext = format!("{}{}", drawtext, date);
        in_out.parse(&drawtext).expect("Failed to parse drawtext");

        // buffer_ctx.source().add(frame);
        // buffer_sink_ctx.sink().frame(frame);
        unimplemented!();
    }
}
