use std::{
    cell::Cell,
    ffi::CString,
    mem,
    ops::{Deref, DerefMut},
    ptr,
};

use crate::ffmpeg::{FmtCtx, StreamCtx};
use ffmpeg_next::{
    decoder, encoder,
    filter::{self, Context, Filter, Graph, Source},
    format,
    frame::Video,
    picture, Error, Frame, Packet, Rational,
};
use ffmpeg_sys_next::{
    av_buffersink_params_alloc, av_packet_unref, avfilter_graph_create_filter, AVFrame,
    AVPixelFormat, AV_PIX_FMT_YUV420P10,
};
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
            dec_ctx.format() as isize,
            dec_ctx.time_base().numerator(),
            dec_ctx.time_base().denominator(),
            dec_ctx.aspect_ratio().numerator(),
            dec_ctx.aspect_ratio().denominator(),
        );
        println!("dec_ctx init info!!:{}", args);
        // find filter
        let buffesrc = filter::find("buffer").unwrap();
        let buffersink = filter::find("buffersink").unwrap();
        // 创建滤波图
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
        // 初始化
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
        buffersrc_ctx.set_pixel_format(format::Pixel::YUV420P);
        // println!(
        //     "de_frame init info!!: video_size={}x{}:pix_fmt={}:pixel_aspect={}/{}:pts:{:?}",
        //     frame.width(),
        //     frame.height(),
        //     frame.format() as usize,
        //     frame.aspect_ratio().numerator(),
        //     frame.aspect_ratio().denominator(),
        //     frame.pts()
        // );
        unsafe {
            let frame = frame.deref();
            let f = frame.as_ptr() as *mut AVFrame;
            // println!("format:{},pts:{}", (*f).format, (*f).pts);
            (*f).format = 1; // 手动修复format？？？
        }

        println!("传frame到graph中");
        // 传递frame到滤波图中
        buffersrc_ctx
            .source()
            .add(frame)
            .expect("Error while feeding the filtergraph");
        println!("传frame到graph后");
        loop {
            let mut buffersink_ctx = self.filter_graph.as_mut().unwrap().get("out").unwrap();
            let mut buffersink_ctx = buffersink_ctx.sink();
            println!("从graph中取frame");
            let filter_frame = self.filter_frame.as_deref_mut().unwrap();
            match buffersink_ctx.frame(filter_frame) {
                Ok(()) => {
                    println!("buffersink success!!!!!!!!!!!!!!!");
                }
                Err(e) => {
                    println!("取frame出错了:{:?}", e);
                    break;
                }
            };
            self.filter_frame
                .as_mut()
                .unwrap()
                .set_kind(picture::Type::None);
            self.filter_frame = Some(Video::empty());
            match self.encode_write_frame(enc_ctx, fmt_ctx) {
                Ok(()) => {}
                Err(_) => {
                    break;
                }
            };
        }
    }
    pub fn encode_write_frame(
        &mut self,
        enc_ctx: &mut encoder::Video,
        fmt_ctx: &mut FmtCtx,
    ) -> Result<(), Error> {
        enc_ctx.send_frame(self.filter_frame.as_deref().unwrap())?;

        loop {
            let en_packet = self.en_pkt.as_mut().unwrap();
            match enc_ctx.receive_packet(en_packet) {
                Ok(_) => {}
                Err(_) => {
                    break;
                }
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
            en_packet.write(fmt_ctx.out_fmt_ctx.as_mut().unwrap())?;
        }
        Ok(())
    }
}
