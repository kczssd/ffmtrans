use ffmpeg_next::format::Pixel;
use ffmpeg_next::frame::Video;
use ffmpeg_next::{codec, Frame};
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
use std::path::Path;

pub struct FFmpeg<'a> {
    in_path: &'a Path,
    out_path: &'a Path,
    pub in_f_ctx: Option<Input>,   // AVFormatContext
    pub out_f_ctx: Option<Output>, // AVFormatContext
    stream_id: (i32, i32),
    frame_que: Vec<Video>,
}

impl<'a> FFmpeg<'a> {
    pub fn init(in_path: &'a Path, out_path: &'a Path) -> Self {
        // register & network_init
        format::register_all();
        format::network::init();
        FFmpeg {
            in_path,
            out_path,
            in_f_ctx: None,
            stream_id: (-1, -1),
            out_f_ctx: None,
            frame_que: vec![],
        }
    }

    pub fn in_open(&mut self, options: Option<Owned>) {
        self.in_f_ctx = match options {
            Some(op) => Some(
                format::input_with_dictionary(&self.in_path, op)
                    .expect("Failed to open input file"),
            ),
            None => Some(format::input(&self.in_path).expect("Failed to open input file")),
        };
        let in_f_ctx = self.in_f_ctx.as_ref().unwrap();
        for i in 0..in_f_ctx.nb_streams() {
            let stream = in_f_ctx.stream(i as usize).unwrap();
            match stream.codec().medium() {
                Type::Video => {
                    self.stream_id.0 = i as i32;
                    break;
                }
                Type::Audio => {
                    self.stream_id.1 = i as i32;
                    break;
                }
                _ => {}
            }
        }
        input::dump(in_f_ctx, 0, self.in_path.to_str());
    }

    pub fn out_open(&mut self, fmt: &str, options: Option<Owned>) {
        self.out_f_ctx = match options {
            Some(op) => Some(
                format::output_as_with(&self.out_path, fmt, op).expect("Failed to open output URL"),
            ),
            None => {
                Some(format::output_as(&self.out_path, fmt).expect("Failed to open output URL"))
            }
        };
        let out_f_ctx_mut = self.out_f_ctx.as_mut().unwrap();
        let in_f_ctx = self.in_f_ctx.as_ref().unwrap();

        unsafe {
            for i in 0..in_f_ctx.nb_streams() {
                let input_stream = in_f_ctx.stream(i as usize).unwrap(); // stream.codec() => AVCodecContext
                let output_stream = out_f_ctx_mut
                    .add_stream(input_stream.codec().codec()) // stream.codec().codec() => AVCodec
                    .expect("Failed add output stream");
                output_stream.codec().clone_from(&input_stream.codec()); // avcodec_copy_context
                (*output_stream.codec().as_mut_ptr()).codec_tag = 0;
            }
        }
        output::dump(out_f_ctx_mut, 0, self.out_path.to_str());
    }

    pub fn decoder(&mut self) {
        // open decoder
        let in_codec_ctx = self
            .in_f_ctx
            .as_mut()
            .unwrap()
            .stream(self.stream_id.0 as usize)
            .unwrap()
            .codec(); // AVCodecContext
        let in_codec = codec::decoder::find(in_codec_ctx.id()).expect("Failed to find codec"); // AVCodec
        let mut in_opened = in_codec_ctx
            .decoder()
            .open_as(in_codec)
            .expect("Failed to open codec");

        // frame&packet
        // input::dump(in_f_ctx_mut, 0, self.in_path.to_str());

        loop {
            let mut packet = Packet::empty(); // AVPacket
            let mut vid_frame = Video::empty(); // AVFrame
            match packet.read(self.in_f_ctx.as_mut().unwrap()) {
                Ok(_) => {}
                Err(_) => break,
            };
            // video packet
            if packet.stream() == self.stream_id.0 as usize {
                in_opened
                    .send_packet(&packet)
                    .expect("Failed to send packet");
                in_opened
                    .receive_frame(&mut vid_frame)
                    .expect("Failed to receive frame");
                self.enqueue(vid_frame);
            }
        }
    }
    pub fn encoder(&mut self) {
        // output_stream 如果不clone_from 需要单独设置
        // open encoder
        let out_codec_ctx = self
            .out_f_ctx
            .as_mut()
            .unwrap()
            .stream(self.stream_id.0 as usize)
            .unwrap()
            .codec(); // AVCodecContext

        let out_codec = codec::encoder::find(out_codec_ctx.id()).expect("Failed to find codec"); // AVCodec
        let mut options = Owned::new();
        options.set("preset", "slow");
        options.set("tune", "zerolatency");
        let mut out_encoder = out_codec_ctx.encoder().video().unwrap();
        // configure
        let decoder = self
            .in_f_ctx
            .as_ref()
            .unwrap()
            .stream(self.stream_id.0 as usize)
            .unwrap()
            .codec()
            .decoder()
            .video()
            .unwrap();
        out_encoder.set_format(Pixel::YUV420P);
        out_encoder.set_frame_rate(decoder.frame_rate());
        out_encoder.set_width(decoder.width());
        out_encoder.set_height(decoder.height());
        out_encoder.set_time_base(decoder.time_base());
        out_encoder.set_bit_rate(decoder.bit_rate());
        out_encoder.set_qmin(10);
        out_encoder.set_qmax(51);
        out_encoder.set_me_range(16);
        out_encoder.set_me_range(16);
        let mut out_opened = out_encoder
            .open_as_with(out_codec, options)
            .expect("Failed to open encoder");
        // println!("w{}h{}", decoder.width(), decoder.height());
        self.write_header();

        for frame in self.frame_que.iter() {
            let mut packet = Packet::empty(); // AVPacket
            out_opened.send_frame(frame).expect("Failed to send frame");
            out_opened
                .receive_packet(&mut packet)
                .expect("Failed to receive packet");
            packet
                .write(self.out_f_ctx.as_mut().unwrap())
                .expect("Failed to write packet");
        }
        self.write_trailer();
    }

    pub fn remuxer_stream(&mut self) {
        // write file header
        self.write_header();

        let mut frame_idx = 0;
        let start_time = current();
        let mut old_dts: Option<i64> = None;

        let mut in_f_ctx_mut = self.in_f_ctx.as_mut().unwrap();
        let mut out_f_ctx_mut = self.out_f_ctx.as_mut().unwrap();

        loop {
            let mut packet = Packet::empty();
            match packet.read(&mut in_f_ctx_mut) {
                Ok(_) => {}
                Err(_) => break,
            };
            // pts dts duration
            if packet.pts().is_none() || packet.dts().is_none() {
                //Write PTS
                let stream = in_f_ctx_mut.stream(self.stream_id.0 as usize).unwrap();
                let time_base = stream.time_base();
                //Duration between 2 frames (us)
                let duration = AV_TIME_BASE as i64 / f64::from(stream.rate()) as i64;
                //Parameters
                packet.set_pts(Some(
                    ((frame_idx * duration) as f64 / (f64::from(time_base) * AV_TIME_BASE as f64))
                        as i64,
                ));
                packet.set_dts(packet.pts());
                packet.set_duration(
                    (duration as f64 / (f64::from(time_base) * AV_TIME_BASE as f64)) as i64,
                )
            }
            // delay
            if packet.stream() == self.stream_id.0 as usize {
                let time_base = in_f_ctx_mut
                    .stream(self.stream_id.0 as usize)
                    .unwrap()
                    .time_base();
                let time_base_q = Rational::new(1, AV_TIME_BASE);
                let pts_time = packet.dts().unwrap().rescale(time_base, time_base_q);
                let now_time = current() - start_time;
                if pts_time > now_time {
                    time::sleep((pts_time - now_time) as u32).expect("Failed to sleep");
                }
            }
            let in_stream = in_f_ctx_mut.stream(packet.stream()).unwrap();
            let out_stream = out_f_ctx_mut.stream(packet.stream()).unwrap();
            // copy packet
            packet.set_pts(Some(packet.pts().unwrap().rescale_with(
                in_stream.time_base(),
                out_stream.time_base(),
                Rounding::NearInfinity,
            )));
            packet.set_dts(Some(packet.dts().unwrap().rescale_with(
                in_stream.time_base(),
                out_stream.time_base(),
                Rounding::NearInfinity,
            )));
            packet.set_duration(packet.duration().rescale_with(
                in_stream.time_base(),
                out_stream.time_base(),
                Rounding::NearInfinity,
            ));
            packet.set_position(-1);
            if old_dts.is_some() && old_dts.unwrap() > packet.dts().unwrap() {
                packet.set_pts(Some(old_dts.unwrap() + packet.duration()));
                packet.set_dts(packet.pts());
            }
            // handle error frame dts
            old_dts = packet.dts();
            if packet.stream() == self.stream_id.0 as usize {
                println!("Send {} video frames to output URL\n", frame_idx);
                frame_idx += 1;
            }
            match packet.write(&mut out_f_ctx_mut) {
                Ok(_) => {}
                Err(_) => break,
            };
        }
        self.write_trailer();
    }
}

impl<'a> FFmpeg<'a> {
    fn enqueue(&mut self, frame: Video) {
        self.frame_que.push(frame);
    }
    fn write_header(&mut self) {
        let mut options = Owned::new();
        options.set("flvflags", "no_duration_filesize");
        self.out_f_ctx
            .as_mut()
            .unwrap()
            .write_header_with(options)
            .expect("Failed to write output header");
    }
    fn write_trailer(&mut self) {
        self.out_f_ctx
            .as_mut()
            .unwrap()
            .write_trailer()
            .expect("Failed to write output trailer");
    }
}
