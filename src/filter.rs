use std::cell::Cell;

use crate::ffmpeg::{FmtCtx, StreamCtx};
use ffmpeg_next::{
    decoder, encoder,
    filter::{self, Context, Filter, Graph, Source},
    format,
    frame::Video,
    picture, Frame, Packet, Rational,
};
use ffmpeg_sys_next::av_packet_unref;
#[derive(Default)]
pub struct FilterCtx {
    filter_graph: Option<Graph>,
    en_pkt: Option<Packet>,      //AVPacket
    filter_frame: Option<Video>, //AVFrame
    pub fmt_ctx: FmtCtx,
}

impl FilterCtx {
    pub fn init_filter(&mut self, dec_ctx: &decoder::video::Video) {
        //init
        filter::register_all();
        let args = format!(
            "video_size={}x{}:pix_fmt={}:time_base={}/{}:pixel_aspect={}/{}",
            dec_ctx.width(),
            dec_ctx.height(),
            dec_ctx.format() as usize,
            dec_ctx.time_base().numerator(),
            dec_ctx.time_base().denominator(),
            dec_ctx.aspect_ratio().numerator(),
            dec_ctx.aspect_ratio().denominator(),
        );
        let buffesrc = filter::find("buffer").unwrap();
        let buffersink = filter::find("buffersink").unwrap();
        let mut filter_graph = filter::Graph::new();
        // filter context
        let buffersrc_ctx = filter_graph.add(&buffesrc, "in", &args).unwrap().source();
        let buffersink_ctx = filter_graph.add(&buffersink, "out", "").unwrap().source();
        // parser
        let parser = filter::graph::Parser::new(&mut filter_graph);
        let parser = parser.input("out", 0).unwrap();
        let parser = parser.output("in", 0).unwrap();
        // drawtext
        let drawtext = "drawtext=fontcolor=red:fontsize=50:x=0:y=0:text=";
        let date = "%{localtime\\:%a %b %d %Y}";
        let drawtext = format!("{}{}", drawtext, date);
        parser.parse(&drawtext).expect("Failed to parse drawtext");
        //
        self.filter_graph = Some(filter_graph);
        self.en_pkt = Some(Packet::empty());
        self.filter_frame = Some(Video::empty());
    }
    pub fn filter_encode_write_frame(
        &mut self,
        frame: &Video,
        enc_ctx: &mut encoder::Video,
        fmt_ctx: &mut FmtCtx,
    ) {
        let mut buffersrc_ctx = self.filter_graph.as_mut().unwrap().get("in").unwrap();
        println!("add frame");
        buffersrc_ctx.set_pixel_format(format::Pixel::YUV420P);
        buffersrc_ctx.source().add(frame).unwrap();
        loop {
            let mut buffersink_ctx = self.filter_graph.as_mut().unwrap().get("out").unwrap();
            buffersink_ctx.set_pixel_format(format::Pixel::YUV420P);
            let mut buffersink_ctx = buffersink_ctx.sink();
            buffersink_ctx
                .frame(self.filter_frame.as_mut().unwrap())
                .unwrap();
            self.filter_frame
                .as_mut()
                .unwrap()
                .set_kind(picture::Type::None);
            self.encode_write_frame(enc_ctx, fmt_ctx);
            self.filter_frame = Some(Video::empty());
        }
    }
    pub fn encode_write_frame(&mut self, enc_ctx: &mut encoder::Video, fmt_ctx: &mut FmtCtx) {
        enc_ctx
            .send_frame(self.filter_frame.as_deref().unwrap())
            .expect("Failed to send frame");

        loop {
            let en_packet = self.en_pkt.as_mut().unwrap();
            match enc_ctx.receive_packet(en_packet) {
                Ok(_) => {}
                Err(_) => {}
            };
            unsafe {
                en_packet.set_stream(0);
                en_packet.rescale_ts(
                    (*enc_ctx.as_mut_ptr()).time_base,
                    fmt_ctx
                        .out_fmt_ctx
                        .as_ref()
                        .unwrap()
                        .stream(0)
                        .unwrap()
                        .time_base(),
                )
            }
            // mux
            match en_packet.write(fmt_ctx.out_fmt_ctx.as_mut().unwrap()) {
                Ok(_) => {}
                Err(e) => {
                    println!("{}", e);
                    break;
                }
            };
        }
    }
}
