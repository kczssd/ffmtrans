use ffmpeg_next::decoder::Opened;
use ffmpeg_next::format::Pixel;
use ffmpeg_next::frame::{Audio, Frame, Video};
use ffmpeg_next::{codec, decoder, encoder, Codec};
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
use ffmpeg_sys_next::{
    avcodec_parameters_copy, avcodec_parameters_from_context, avio_open, AVIO_FLAG_WRITE,
    AV_TIME_BASE,
};
use std::ffi::CString;
use std::ops::Deref;
use std::path::Path;
use std::ptr;

#[derive(Default)]
pub struct FmtCtx {
    pub in_fmt_ctx: Option<Input>,   // AVFormatContext
    pub out_fmt_ctx: Option<Output>, // AVFormatContext
}
// change to temple
#[derive(Default)]
pub struct StreamCtx {
    pub dec_ctx: Option<decoder::Video>, //AVCodecContext
    pub enc_ctx: Option<encoder::Video>,
    pub de_frame: Option<Video>, //AVFrame
    pub stream_idx: (u32, u32),
    pub fmt_ctx: FmtCtx,
}

impl StreamCtx {
    pub fn new() -> Self {
        StreamCtx::default()
    }
    pub fn input_open(&mut self, file_path: &Path, options: Option<Owned>) {
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
            let dec = decoder::find(stream.codec().id()).unwrap();
            let codec_ctx = stream.codec();
            match codec_ctx.medium() {
                Type::Video => {
                    self.dec_ctx = Some(codec_ctx.decoder().open_as(dec).unwrap().video().unwrap());
                    self.de_frame = Some(Video::empty());
                    self.stream_idx.0 = i;
                }
                Type::Audio => {
                    self.stream_idx.1 = i;
                }
                _ => {}
            }
        }
        // 打印输入流信息
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
            let mut out_stream = self
                .fmt_ctx
                .out_fmt_ctx
                .as_mut()
                .unwrap()
                .add_stream(unsafe { Codec::wrap(ptr::null_mut()) }) // stream.codec().codec() => AVCodec
                .expect("Failed add output stream");
            match in_stream.codec().medium() {
                Type::Video => {
                    let dec_ctx = self.dec_ctx.as_ref().unwrap();
                    let codec = codec::encoder::find(dec_ctx.id()).expect("Failed to find codec"); // AVCodec
                    let mut enc_ctx = out_stream.codec().encoder().video().unwrap();
                    // configure
                    enc_ctx.set_format(Pixel::YUV420P);
                    enc_ctx.set_width(dec_ctx.width());
                    enc_ctx.set_height(dec_ctx.height());
                    enc_ctx.set_time_base(Rational::new(
                        dec_ctx.frame_rate().unwrap().denominator(),
                        dec_ctx.frame_rate().unwrap().numerator(),
                    ));
                    enc_ctx.set_frame_rate(dec_ctx.frame_rate());
                    enc_ctx.set_bit_rate(50 * 1024 * 8);
                    enc_ctx.set_max_b_frames(0);
                    enc_ctx.set_gop(50);
                    //fix h264 setting
                    enc_ctx.set_qmin(10);
                    enc_ctx.set_qmax(51);
                    enc_ctx.set_me_range(16);
                    let enc_ctx = enc_ctx.open_as(codec).unwrap();
                    unsafe {
                        avcodec_parameters_from_context(
                            (*out_stream.as_mut_ptr()).codecpar,
                            enc_ctx.as_ptr(),
                        );
                        out_stream.set_time_base((*enc_ctx.as_ptr()).time_base);
                    }
                    self.enc_ctx = Some(enc_ctx);
                }
                Type::Audio => unsafe {
                    avcodec_parameters_copy(
                        (*out_stream.as_mut_ptr()).codecpar,
                        (*in_stream.as_ptr()).codecpar,
                    );
                    out_stream.set_time_base(in_stream.time_base());
                    (*(*out_stream.as_mut_ptr()).codecpar).codec_tag = 0;
                },
                _ => {}
            }
            // 打印输出流信息
            output::dump(
                &self.fmt_ctx.out_fmt_ctx.as_ref().unwrap(),
                0,
                file_path.to_str(),
            );
        }
    }
}
