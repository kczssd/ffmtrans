use ffmpeg_next::{
    dictionary::Owned,
    format::{
        self,
        context::{input, output, Input, Output},
    },
    media::Type,
    packet::Mut,
    time::{self, current},
    Packet, Rational, Rescale, Rounding,
};
use ffmpeg_sys_next::{av_free_packet, AV_TIME_BASE};
use std::path::Path;

pub struct FFmpeg<'a> {
    in_path: &'a Path,
    out_path: &'a Path,
    pub in_f_ctx: Option<Input>,   // AVFormatContext
    pub out_f_ctx: Option<Output>, // AVFormatContext
    stream_id: (i32, i32),
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
        output::dump(self.out_f_ctx.as_ref().unwrap(), 0, self.out_path.to_str());
    }

    fn write_header(&mut self) {
        self.out_f_ctx
            .as_mut()
            .unwrap()
            .write_header()
            .expect("Failed to write output header");
    }
    fn write_trailer(&mut self) {
        self.out_f_ctx
            .as_mut()
            .unwrap()
            .write_trailer()
            .expect("Failed to write output trailer");
    }

    pub fn remuxer(&mut self) {
        // write file header
        self.write_header();

        let mut frame_idx = 0;
        let start_time = current();

        loop {
            let mut packet = Packet::empty();
            match packet.read(&mut self.in_f_ctx.as_mut().unwrap()) {
                Ok(_) => {}
                Err(_) => break,
            };
            // pts dts duration
            // delay
            if packet.stream() == self.stream_id.0 as usize {
                let time_base = self
                    .in_f_ctx
                    .as_ref()
                    .unwrap()
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
            let in_stream = self
                .in_f_ctx
                .as_ref()
                .unwrap()
                .stream(packet.stream())
                .unwrap();
            let out_stream = self
                .out_f_ctx
                .as_ref()
                .unwrap()
                .stream(packet.stream())
                .unwrap();
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
            if packet.stream() == self.stream_id.0 as usize {
                println!("Send {} video frames to output URL\n", frame_idx);
                frame_idx += 1;
            }
            match packet.write(&mut self.out_f_ctx.as_mut().unwrap()) {
                Ok(_) => {}
                Err(_) => break,
            };
            unsafe {
                av_free_packet(packet.as_mut_ptr());
            }
        }
        self.write_trailer();
    }
}
