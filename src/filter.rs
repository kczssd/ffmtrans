use std::{
    ffi::c_void,
    mem::size_of,
    ops::{Deref, DerefMut},
    ptr,
};

use crate::ffmpeg::FmtCtx;
use ffmpeg_next::{
    decoder, encoder,
    filter::{self, Graph},
    format::{self, Pixel},
    frame::Video,
    option::Settable,
    picture, Packet,
};
use ffmpeg_sys_next::{avfilter_graph_config, AVFrame};

pub struct FilterCtx {
    filter_graph: Graph,
    filter_frame: Video, //AVFrame
}

impl FilterCtx {
    pub fn init_filter(dec_ctx: &decoder::video::Video) -> Self {
        // 创建滤波图
        let mut filter_graph = filter::Graph::new();
        // 配置滤镜图
        let args = format!(
            "video_size={}x{}:pix_fmt={}:time_base={}/{}:pixel_aspect={}/{}",
            dec_ctx.width(),
            dec_ctx.height(),
            // 修复Changing video frame properties on the fly is not supported by all filters.
            dec_ctx.format() as isize - 1,
            dec_ctx.time_base().numerator(),
            dec_ctx.time_base().denominator(),
            dec_ctx.aspect_ratio().numerator(),
            dec_ctx.aspect_ratio().denominator(),
        );
        // 配置滤镜实例
        let buffesrc: ffmpeg_next::Filter = filter::find("buffer").unwrap();
        let buffersink = filter::find("buffersink").unwrap();
        let mut buffersrc_ctx = filter_graph.add(&buffesrc, "in", &args).unwrap();
        buffersrc_ctx.set_pixel_format(Pixel::YUV420P);
        let mut buffersink_ctx = filter_graph.add(&buffersink, "out", "").unwrap();
        buffersink_ctx.set_pixel_format(Pixel::YUV420P);
        // parser
        let parser = filter::graph::Parser::new(&mut filter_graph);
        let parser = parser.output("in", 0).unwrap();
        let parser = parser.input("out", 0).unwrap();
        // drawtext
        let drawtext = "drawtext=fontcolor=red:fontsize=50:x=0:y=0:text=";
        let date = "'%{localtime\\:%a %b %d %Y}'";
        let drawtext = format!("{}{}", drawtext, date);
        parser.parse(&drawtext).expect("Failed to parse drawtext");
        unsafe {
            avfilter_graph_config(filter_graph.as_mut_ptr(), ptr::null_mut());
        }
        // 初始化
        FilterCtx {
            filter_graph,
            filter_frame: Video::new(Pixel::YUV420P, 1280, 800),
        }
    }

    pub fn filter_encode_write_frame(
        &mut self,
        frame: &mut Video,
        enc_ctx: &mut encoder::Video,
        fmt_ctx: &mut FmtCtx,
    ) {
        let mut buffersrc_ctx = self.filter_graph.get("in").unwrap();
        // println!(
        //     "de_frame init info!!: video_size={}x{}:pix_fmt={}:pixel_aspect={}/{}:pts:{:?}:flags:{:?}",
        //     frame.width(),
        //     frame.height(),
        //     frame.format() as usize,
        //     frame.aspect_ratio().numerator(),
        //     frame.aspect_ratio().denominator(),
        //     frame.pts(),
        //     frame.flags()
        // );

        // 传递frame到滤波图中
        buffersrc_ctx
            .source()
            .add(frame)
            .expect("Error while feeding the filtergraph");
        loop {
            let mut buffersink_ctx = self.filter_graph.get("out").unwrap();

            let mut buffersink_ctx = buffersink_ctx.sink();
            let filter_frame = self.filter_frame.deref_mut();
            match buffersink_ctx.frame(filter_frame) {
                Ok(()) => {
                    println!("buffersink success!!!!!!!!!!!!!!!");
                }
                Err(e) => {
                    println!("取frame出错了:{:?}", e);
                    break;
                }
            };
            self.filter_frame.set_kind(picture::Type::None);
            match self.encode_write_frame(enc_ctx, fmt_ctx) {
                Ok(()) => self.filter_frame = Video::new(Pixel::YUV420P, 1280, 800),
                Err(_) => {
                    break;
                }
            };
        }
        println!("---------------------finish one frame---------------------");
    }
    pub fn encode_write_frame(
        &mut self,
        enc_ctx: &mut encoder::Video,
        fmt_ctx: &mut FmtCtx,
    ) -> Result<(), &str> {
        let mut en_pkg = Packet::empty();
        match enc_ctx.send_frame(self.filter_frame.deref()) {
            Ok(_) => {
                println!("send_frame success,pts:{:?}", self.filter_frame.pts());
            }
            Err(_) => return Err("send_frame failed"),
        };
        loop {
            match enc_ctx.receive_packet(&mut en_pkg) {
                Ok(_) => {
                    println!("receive_packet success");
                }
                Err(_) => return Err("receive_packet failed"),
            };
            // mux
            match en_pkg.write(&mut fmt_ctx.out_fmt_ctx) {
                Ok(_) => {
                    println!("write frame success");
                }
                Err(_) => {
                    println!("write frame failed");
                    return Err("write frame failed");
                }
            };
        }
    }
}
