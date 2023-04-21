use ffmpeg_next::codec::Context;
use ffmpeg_next::format::Pixel;
use ffmpeg_next::frame::Video;
use ffmpeg_next::{codec, decoder, encoder, Codec};
use ffmpeg_next::{
    dictionary::Owned,
    format::{
        self,
        context::{input, output, Input, Output},
    },
    media::Type,
    Rational,
};
use ffmpeg_sys_next::{avcodec_parameters_copy, avcodec_parameters_from_context};
use std::path::Path;
use std::ptr;

pub struct FmtCtx {
    pub in_fmt_ctx: Input,   // AVFormatContext
    pub out_fmt_ctx: Output, // AVFormatContext
}
// change to temple
pub struct StreamCtx {
    pub dec_ctx: decoder::Video, //AVCodecContext
    pub enc_ctx: encoder::Video,
    pub de_frame: Video, //AVFrame
    pub stream_idx: (u32, u32),
    pub fmt_ctx: FmtCtx,
}

impl StreamCtx {
    pub fn init(
        in_path: &Path,
        in_config: Option<Owned>,
        out_path: &Path,
        fmt: &str,
        out_config: Option<Owned>,
    ) -> Self {
        let (in_fmt_ctx, dec_ctx, stream_idx) = StreamCtx::input_open(in_path, in_config);
        let (out_fmt_ctx, enc_ctx) =
            StreamCtx::out_open(out_path, fmt, out_config, &in_fmt_ctx, &dec_ctx);
        StreamCtx {
            dec_ctx,
            enc_ctx,
            de_frame: Video::new(Pixel::YUV420P, 1280, 800),
            stream_idx,
            fmt_ctx: FmtCtx {
                in_fmt_ctx,
                out_fmt_ctx,
            },
        }
    }
    pub fn input_open(
        file_path: &Path,
        options: Option<Owned>,
    ) -> (Input, decoder::Video, (u32, u32)) {
        let in_fmt_ctx = match options {
            Some(op) => {
                format::input_with_dictionary(&file_path, op).expect("Failed to open input file")
            }
            None => format::input(&file_path).expect("Failed to open input file"),
        };
        let mut dec_ctx = None;
        let mut stream_idx = (0, 0);
        for i in 0..in_fmt_ctx.nb_streams() {
            let stream = in_fmt_ctx.stream(i as usize).unwrap();
            let parameters = stream.parameters();
            let codec_ctx = Context::from_parameters(parameters).unwrap();
            match stream.parameters().medium() {
                Type::Video => {
                    let mut opened_ctx = codec_ctx.decoder();
                    unsafe {
                        (*opened_ctx.as_mut_ptr()).framerate = Rational::new(30, 1).into();
                        (*opened_ctx.as_mut_ptr()).time_base = Rational::new(1, 30).into();
                        (*opened_ctx.as_mut_ptr()).sample_aspect_ratio =
                            Rational::new(1280, 800).into();
                    }
                    dec_ctx = Some(opened_ctx.video().unwrap());
                    stream_idx.0 = i;
                }
                Type::Audio => {
                    stream_idx.1 = i;
                }
                _ => {}
            }
        }
        // 打印输入流信息
        input::dump(&in_fmt_ctx, 0, file_path.to_str());
        (in_fmt_ctx, dec_ctx.unwrap(), stream_idx)
    }
    pub fn out_open(
        file_path: &Path,
        fmt: &str,
        options: Option<Owned>,
        in_fmt_ctx: &Input,
        dec_ctx: &decoder::Video,
    ) -> (Output, encoder::Video) {
        let mut out_fmt_ctx = match options {
            Some(op) => {
                format::output_as_with(&file_path, fmt, op).expect("Failed to open output URL")
            }
            None => format::output_as(&file_path, fmt).expect("Failed to open output URL"),
        };
        let mut enc_ctx = None;
        for i in 0..in_fmt_ctx.nb_streams() {
            let in_stream = in_fmt_ctx.stream(i as usize).unwrap(); // stream.codec() => AVCodecContext
            let mut out_stream = out_fmt_ctx
                .add_stream(unsafe { Codec::wrap(ptr::null_mut()) }) // stream.codec().codec() => AVCodec
                .expect("Failed add output stream");
            let parameters = in_stream.parameters();
            match parameters.medium() {
                Type::Video => {
                    let codec = codec::encoder::find(dec_ctx.id()).expect("Failed to find codec"); // AVCodec
                    let mut opened_ctx = Context::from_parameters(out_stream.parameters())
                        .unwrap()
                        .encoder()
                        .video()
                        .unwrap();
                    // configure
                    opened_ctx.set_format(Pixel::YUV420P);
                    opened_ctx.set_width(dec_ctx.width());
                    opened_ctx.set_height(dec_ctx.height());
                    opened_ctx.set_time_base(Rational::new(
                        dec_ctx.frame_rate().unwrap().denominator(),
                        dec_ctx.frame_rate().unwrap().numerator(),
                    ));
                    opened_ctx.set_frame_rate(dec_ctx.frame_rate());
                    // opened_ctx.set_bit_rate(50 * 1024 * 8);
                    opened_ctx.set_max_b_frames(0);
                    // opened_ctx.set_gop(50);
                    //fix h264 setting
                    opened_ctx.set_qmin(10);
                    opened_ctx.set_qmax(51);
                    opened_ctx.set_me_range(16);
                    let opened_ctx = opened_ctx.open_as(codec).unwrap();
                    unsafe {
                        avcodec_parameters_from_context(
                            (*out_stream.as_mut_ptr()).codecpar,
                            opened_ctx.as_ptr(),
                        );
                        out_stream.set_time_base((*opened_ctx.as_ptr()).time_base);
                    }
                    // println!(
                    //     "dec_ctx {:?}{:?},enc_ctx {:?}{:?},width:{},height:{}",
                    //     dec_ctx.frame_rate(),
                    //     dec_ctx.time_base(),
                    //     unsafe { (*opened_ctx.as_ptr()).framerate },
                    //     unsafe { (*opened_ctx.as_ptr()).time_base },
                    //     opened_ctx.width(),
                    //     opened_ctx.height()
                    // );
                    enc_ctx = Some(opened_ctx);
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
        }
        // 打印输出流信息
        output::dump(&out_fmt_ctx, 0, file_path.to_str());
        (out_fmt_ctx, enc_ctx.unwrap())
    }
}
