use std::ops::{Deref, DerefMut};

use crate::{ffmpeg::FmtCtx, TimeGap};
use ffmpeg_next::{
    decoder, encoder,
    filter::{self, Graph},
    format::Pixel,
    frame::Video,
    picture, Packet,
};

pub struct FilterCtx {
    filter_graph: Graph,
    filter_frame: Video, // AVFrame
}

impl FilterCtx {
    pub fn init_filter(dec_ctx: &decoder::video::Video) -> Self {
        // create filter graph
        let mut filter_graph = filter::Graph::new();
        // init filter context
        let args = format!(
            "video_size={}x{}:pix_fmt={}:time_base={}/{}:pixel_aspect={}/{}",
            dec_ctx.width(),
            dec_ctx.height(),
            // fix "Changing video frame properties on the fly is not supported by all filters."
            dec_ctx.format() as isize - 1,
            dec_ctx.time_base().numerator(),
            dec_ctx.time_base().denominator(),
            dec_ctx.aspect_ratio().numerator(),
            dec_ctx.aspect_ratio().denominator(),
        );
        let buffesrc: ffmpeg_next::Filter = filter::find("buffer").unwrap();
        let buffersink = filter::find("buffersink").unwrap();
        let mut buffersrc_ctx = filter_graph.add(&buffesrc, "in", &args).unwrap();
        buffersrc_ctx.set_pixel_format(Pixel::YUV420P);
        let mut buffersink_ctx = filter_graph.add(&buffersink, "out", "").unwrap();
        buffersink_ctx.set_pixel_format(Pixel::YUV420P);
        // init parser
        let parser = filter::graph::Parser::new(&mut filter_graph);
        let parser = parser.output("in", 0).unwrap();
        let parser = parser.input("out", 0).unwrap();
        // filter description
        let drawtext = "drawtext=fontcolor=red:fontsize=20:x=0:y=0:text=";
        let date = "'%{localtime\\:%Y-%m-%d %H.%M.%S}'";
        let drawtext = format!("{}{}", drawtext, date);
        parser.parse(&drawtext).expect("Failed to parse drawtext");
        // connect filters
        filter_graph.validate().expect("Failed to connect filters");
        FilterCtx {
            filter_graph,
            filter_frame: Video::new(Pixel::YUV420P, dec_ctx.width(), dec_ctx.height()),
        }
    }

    pub fn filter_encode_write_frame(
        &mut self,
        frame: &mut Video,
        enc_ctx: &mut encoder::Video,
        fmt_ctx: &mut FmtCtx,
        time_gap: &mut TimeGap,
    ) {
        // send frame to filter graph
        let mut buffersrc_ctx = self.filter_graph.get("in").unwrap();
        buffersrc_ctx
            .source()
            .add(frame)
            .expect("Error while feeding the filter_graph");
        loop {
            // get frame from filter graph
            let mut buffersink_ctx = self.filter_graph.get("out").unwrap();
            let mut buffersink_ctx = buffersink_ctx.sink();
            let filter_frame = self.filter_frame.deref_mut();
            match buffersink_ctx.frame(filter_frame) {
                Ok(()) => {}
                Err(e) => {
                    println!("get frame failed from buffersink:{:?}", e);
                    break;
                }
            };
            self.filter_frame.set_kind(picture::Type::None);
            // mux frame
            match self.encode_write_frame(enc_ctx, fmt_ctx, time_gap) {
                Ok(()) => {
                    self.filter_frame = Video::new(Pixel::YUV420P, frame.width(), frame.height())
                }
                Err(_) => {
                    self.filter_frame = Video::new(Pixel::YUV420P, frame.width(), frame.height());
                    break;
                }
            };
        }
    }

    pub fn encode_write_frame(
        &mut self,
        enc_ctx: &mut encoder::Video,
        fmt_ctx: &mut FmtCtx,
        time_gap: &mut TimeGap,
    ) -> Result<(), &str> {
        let mut en_pkt = Packet::empty();
        match enc_ctx.send_frame(self.filter_frame.deref()) {
            Ok(_) => {
                // println!("send_frame success");
            }
            Err(_) => return Err("send_frame failed"),
        };
        loop {
            match enc_ctx.receive_packet(&mut en_pkt) {
                Ok(_) => {
                    // println!("receive_packet success");
                }
                Err(_) => return Err("receive_packet failed"),
            };
            en_pkt.set_stream(0);
            let out_fmt_timebase = fmt_ctx.out_fmt_ctx.stream(0).unwrap().time_base();
            unsafe {
                en_pkt.rescale_ts((*enc_ctx.as_ptr()).time_base, out_fmt_timebase);
            }
            en_pkt.set_dts(Some(
                (time_gap.audio_time / f64::from(out_fmt_timebase)) as i64,
            ));
            en_pkt.set_pts(Some(
                (time_gap.audio_time / f64::from(out_fmt_timebase)) as i64,
            ));
            let video_time: f64 = en_pkt.pts().unwrap() as f64 * f64::from(out_fmt_timebase);
            time_gap.video_time = video_time;
            // write to stream
            match en_pkt.write(&mut fmt_ctx.out_fmt_ctx) {
                Ok(_) => {
                    // println!("----write packet success----");
                }
                Err(_) => {
                    // println!("write frame failed");
                    return Err("write frame failed");
                }
            };
        }
    }
}
