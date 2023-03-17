extern crate ffmpeg_next;

use ffmpeg_next::codec::{parameters, Context};
use ffmpeg_next::format::context::{input, output};
use ffmpeg_next::frame::Video;
use ffmpeg_next::media::Type;
use ffmpeg_next::packet::Mut;
use ffmpeg_next::time::{self, current};
use ffmpeg_next::util::format::pixel::Pixel;
use ffmpeg_next::{codec, dictionary, format, util, Packet, Rational, Rescale, Rounding, Stream};
use ffmpeg_sys_next::{
    av_free_packet, av_gettime, av_q2d, av_rescale_q, av_usleep, avcodec_copy_context,
    AV_NOPTS_VALUE, AV_TIME_BASE,
};
use std::env;
use std::path::Path;

fn main() {
    // register
    format::register_all();
    format::network::init();
    // params
    let mut video_stream_index = 0;
    let mut frame_index = 0;
    // Parse command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        panic!("Usage: ./mp4_to_rtmp <filename.mp4> <rtmp://stream-url>");
    }
    let input_path = Path::new(&args[1]);
    let output_url = &args[2];

    // Open input file using FFmpeg
    let mut options = dictionary::Owned::new();
    let mut input_format_context =
        format::input_with_dictionary(&input_path, options).expect("Failed to open input file"); // AVFormatContext

    for i in 0..input_format_context.nb_streams() {
        let stream = input_format_context.stream(i as usize).unwrap();
        match stream.codec().medium() {
            Type::Video => {
                // Find video stream in the input file
                video_stream_index = i;
                break;
            }
            Type::Audio => {}
            _ => {}
        }
    }
    input::dump(&input_format_context, 0, input_path.to_str());

    // Open output URL using FFmpeg
    let mut output_format_context =
        format::output_as(output_url, "flv").expect("Failed to open output URL"); // AVFormatContext
    let output_format = output_format_context.format(); // AVOutputFormat

    unsafe {
        for i in 0..input_format_context.nb_streams() {
            let input_stream = input_format_context.stream(i as usize).unwrap(); // stream.codec() => AVCodecContext
            let output_stream = output_format_context
                .add_stream(input_stream.codec().codec()) // stream.codec().codec() => AVCodec
                .expect("Failed add output stream");
            output_stream.codec().clone_from(&input_stream.codec()); // avcodec_copy_context
            (*output_stream.codec().as_mut_ptr()).codec_tag = 0;
        }
    }
    output::dump(&output_format_context, 0, Some(&output_url));

    // write file header
    output_format_context
        .write_header()
        .expect("Failed to write output header");
    println!("asdf");
    let start_time = current();
    loop {
        let mut packet = Packet::empty();
        match packet.read(&mut input_format_context) {
            Ok(_) => {}
            Err(_) => break,
        };
        // pts dts duration
        // delay
        if packet.stream() == video_stream_index as usize {
            let time_base = input_format_context
                .stream(video_stream_index as usize)
                .unwrap()
                .time_base();
            let time_base_q = Rational::new(1, AV_TIME_BASE);
            let pts_time = packet.dts().unwrap().rescale(time_base, time_base_q);
            let now_time = current() - start_time;
            if pts_time > now_time {
                time::sleep((pts_time - now_time) as u32).expect("Failed to sleep");
            }
        }
        let in_stream = input_format_context.stream(packet.stream()).unwrap();
        let out_stream = output_format_context.stream(packet.stream()).unwrap();
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
        if packet.stream() == video_stream_index as usize {
            println!("Send {} video frames to output URL\n", frame_index);
            frame_index += 1;
        }
        match packet.write(&mut output_format_context) {
            Ok(_) => {}
            Err(_) => break,
        };
        unsafe {
            av_free_packet(packet.as_mut_ptr());
        }
    }
    output_format_context
        .write_trailer()
        .expect("Failed to write output trailer");
}
