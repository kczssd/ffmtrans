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
use ffmpeg_sys_next::{av_guess_frame_rate, avcodec_parameters_copy};
use std::path::Path;
use std::ptr;

pub struct FmtCtx {
    pub in_fmt_ctx: Input,   // AVFormatContext
    pub out_fmt_ctx: Output, // AVFormatContext
}

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
        let mut dec_ctx = None;
        let mut stream_idx = (0, 0);

        let in_fmt_ctx = match options {
            Some(op) => {
                format::input_with_dictionary(&file_path, op).expect("Failed to open input file")
            }
            None => format::input(&file_path).expect("Failed to open input file"),
        };

        for i in 0..in_fmt_ctx.nb_streams() {
            let stream = in_fmt_ctx.stream(i as usize).unwrap();
            match stream.parameters().medium() {
                Type::Video => {
                    let parameters = stream.parameters();
                    let codec_ctx = Context::from_parameters(parameters).unwrap();
                    let mut codec_ctx = codec_ctx.decoder();
                    unsafe {
                        (*codec_ctx.as_mut_ptr()).framerate = av_guess_frame_rate(
                            in_fmt_ctx.as_ptr() as *mut _,
                            stream.as_ptr() as *mut _,
                            ptr::null_mut(),
                        );
                    }
                    dec_ctx = Some(codec_ctx.video().unwrap());
                    stream_idx.0 = i;
                }
                Type::Audio => {
                    stream_idx.1 = i;
                }
                _ => {}
            }
        }
        // print input info
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
        let mut enc_ctx = None;

        let mut out_fmt_ctx = match options {
            Some(op) => {
                format::output_as_with(&file_path, fmt, op).expect("Failed to open output URL")
            }
            None => format::output_as(&file_path, fmt).expect("Failed to open output URL"),
        };

        for i in 0..in_fmt_ctx.nb_streams() {
            let in_stream = in_fmt_ctx.stream(i as usize).unwrap();
            let mut out_stream = out_fmt_ctx
                .add_stream(unsafe { Codec::wrap(ptr::null_mut()) })
                .expect("Failed add output stream");
            let parameters = in_stream.parameters();
            match parameters.medium() {
                Type::Video => {
                    let codec = codec::encoder::find(dec_ctx.id()).expect("Failed to find codec");
                    let mut codec_ctx = Context::new().encoder().video().unwrap();
                    // encode context configure
                    codec_ctx.set_height(dec_ctx.height());
                    codec_ctx.set_width(dec_ctx.width());
                    codec_ctx.set_aspect_ratio(dec_ctx.aspect_ratio());
                    codec_ctx.set_frame_rate(dec_ctx.frame_rate());
                    codec_ctx.set_gop(50);
                    codec_ctx.set_max_b_frames(0);
                    codec_ctx.set_format(Pixel::YUV420P);
                    codec_ctx.set_bit_rate(2564 * 1000);
                    codec_ctx.set_time_base(Rational::new(
                        dec_ctx.frame_rate().unwrap().denominator(),
                        dec_ctx.frame_rate().unwrap().numerator(),
                    ));
                    // // fix h264 setting
                    codec_ctx.set_qmin(10);
                    codec_ctx.set_qmax(51);
                    codec_ctx.set_me_range(16);
                    let codec_ctx = codec_ctx.open_as(codec).unwrap();
                    // set out stream
                    unsafe {
                        out_stream.set_parameters(parameters);
                        (*out_stream.as_mut_ptr()).time_base = (*codec_ctx.as_ptr()).time_base;
                    }
                    enc_ctx = Some(codec_ctx);
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
        // print output info
        output::dump(&out_fmt_ctx, 0, file_path.to_str());
        (out_fmt_ctx, enc_ctx.unwrap())
    }
}
