use ffmpeg_next::decoder::Opened;
use ffmpeg_next::format::Pixel;
use ffmpeg_next::frame::{Audio, Frame, Video};
use ffmpeg_next::{codec, decoder, encoder};
use ffmpeg_next::{
    dictionary::Owned,
    format::{
        self,
        context::{input, output, Input, Output},
    },
    media::Type,
    time::{self, current},
    Packet, Rational, Rescale, Rounding,
};
use ffmpeg_sys_next::AV_TIME_BASE;
use std::ops::Deref;
use std::path::Path;

#[derive(Default)]
pub struct FmtCtx {
    pub in_fmt_ctx: Option<Input>,   // AVFormatContext
    pub out_fmt_ctx: Option<Output>, // AVFormatContext
}
// change to temple
#[derive(Default)]
pub struct StreamCtx {
    pub dec_ctx: (Option<decoder::Video>, Option<decoder::Audio>), //AVCodecContext
    pub enc_ctx: (Option<encoder::Video>, Option<encoder::audio::Audio>),
    pub de_frame: (Option<Video>, Option<Audio>), //AVFrame
    pub stream_idx: (u32, u32),
    pub fmt_ctx: FmtCtx,
}

impl StreamCtx {
    pub fn new() -> Self {
        StreamCtx::default()
    }
    pub fn in_open(&mut self, file_path: &Path, options: Option<Owned>) {
        self.fmt_ctx.in_fmt_ctx = match options {
            Some(op) => Some(
                format::input_with_dictionary(&file_path, op).expect("Failed to open input file"),
            ),
            None => Some(format::input(&file_path).expect("Failed to open input file")),
        };
        for i in 0..self.fmt_ctx.in_fmt_ctx.as_ref().unwrap().nb_streams() {
            let stream = self
                .fmt_ctx
                .in_fmt_ctx
                .as_ref()
                .unwrap()
                .stream(i as usize)
                .unwrap();
            let dec = decoder::find(stream.codec().id()).unwrap(); //AVCodec
            let codec_ctx = stream.codec();

            match codec_ctx.medium() {
                Type::Video => {
                    self.dec_ctx.0 =
                        Some(codec_ctx.decoder().open_as(dec).unwrap().video().unwrap());
                    self.de_frame.0 = Some(Video::empty()); // need fmt wh?
                    self.stream_idx.0 = i;
                    println!("video{}", i);
                }
                Type::Audio => {
                    self.dec_ctx.1 =
                        Some(codec_ctx.decoder().open_as(dec).unwrap().audio().unwrap());
                    self.de_frame.1 = Some(Audio::empty());
                    self.stream_idx.1 = i;
                    println!("audio{}", i);
                }
                _ => {}
            }
        }
        input::dump(
            &self.fmt_ctx.in_fmt_ctx.as_ref().unwrap(),
            0,
            file_path.to_str(),
        );
    }
    pub fn out_open(&mut self, file_path: &Path, fmt: &str, options: Option<Owned>) {
        self.fmt_ctx.out_fmt_ctx = match options {
            Some(op) => Some(
                format::output_as_with(&file_path, fmt, op).expect("Failed to open output URL"),
            ),
            None => Some(format::output_as(&file_path, fmt).expect("Failed to open output URL")),
        };
        for i in 0..self.fmt_ctx.in_fmt_ctx.as_ref().unwrap().nb_streams() {
            let in_stream = self
                .fmt_ctx
                .in_fmt_ctx
                .as_ref()
                .unwrap()
                .stream(i as usize)
                .unwrap(); // stream.codec() => AVCodecContext
            let out_stream = self
                .fmt_ctx
                .out_fmt_ctx
                .as_mut()
                .unwrap()
                .add_stream(in_stream.codec().codec()) // stream.codec().codec() => AVCodec
                .expect("Failed add output stream");
            // out_stream.codec().clone_from(&in_stream.codec()); // avcodec_copy_context?
            match in_stream.codec().medium() {
                Type::Video => {
                    let dec_ctx = self.dec_ctx.0.as_ref().unwrap();
                    let codec = codec::encoder::find(dec_ctx.id()).expect("Failed to find codec"); // AVCodec
                    let mut options = Owned::new();
                    options.set("preset", "slow");
                    options.set("tune", "zerolatency");
                    let mut enc_ctx = out_stream.codec().encoder().video().unwrap();
                    // configure
                    enc_ctx.set_format(Pixel::YUV420P);
                    enc_ctx.set_frame_rate(dec_ctx.frame_rate());
                    enc_ctx.set_width(dec_ctx.width());
                    enc_ctx.set_height(dec_ctx.height());
                    enc_ctx.set_time_base(dec_ctx.time_base());
                    enc_ctx.set_bit_rate(dec_ctx.bit_rate());
                    enc_ctx.set_qmin(10);
                    enc_ctx.set_qmax(51);
                    enc_ctx.set_me_range(16);
                    enc_ctx.set_me_range(16);
                    let enc_ctx = enc_ctx.open_as(codec).unwrap();
                    // let enc_ctx = enc_ctx.clone().encoder().video().unwrap(); // need???
                    self.enc_ctx.0 = Some(enc_ctx);
                }
                Type::Audio => {
                    let dec_ctx = self.dec_ctx.1.as_ref().unwrap();
                    let codec = codec::encoder::find(dec_ctx.id()).expect("Failed to find codec"); // AVCodec
                    let mut enc_ctx = out_stream.codec().encoder().audio().unwrap();
                    enc_ctx.set_rate(dec_ctx.rate() as i32);
                    // ch_layout
                    enc_ctx.set_format(dec_ctx.format());
                    enc_ctx.set_time_base(dec_ctx.time_base());
                    enc_ctx.set_channel_layout(dec_ctx.channel_layout());
                    let enc_ctx = enc_ctx.open_as(codec).unwrap();
                    let enc_ctx = enc_ctx.clone().encoder().audio().unwrap(); // ???
                    self.enc_ctx.1 = Some(enc_ctx);
                }
                _ => {}
            }
            output::dump(
                &self.fmt_ctx.out_fmt_ctx.as_ref().unwrap(),
                0,
                file_path.to_str(),
            );
            // init muxer
            // let mut m_op = Owned::new();
            // m_op.set("flvflags", "no_duration_filesize");
            // self.fmt_ctx
            //     .out_fmt_ctx
            //     .as_mut()
            //     .unwrap()
            //     .write_header_with(m_op)
            //     .unwrap();
        }
    }
}
