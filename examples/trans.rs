extern crate ffmpeg_next;

use ffmpeg_next::codec::encoder::Encoder;
use ffmpeg_next::format::context::output::Output;
use ffmpeg_next::frame::Video;
use ffmpeg_next::media::Type;
use ffmpeg_next::util::format::pixel::Pixel;
use ffmpeg_next::util::frame::video::Video as VideoFrame;
use ffmpeg_next::{codec, encoder, format, Frame, Packet};
use std::env;
use std::path::Path;

fn main() {
    format::register_all();
    format::network::init();
    // Parse command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        panic!("Usage: ./mp4_to_rtmp <filename.mp4> <rtmp://stream-url>");
    }
    let input_path = Path::new(&args[1]);
    let output_url = &args[2];

    // Open input file using FFmpeg
    let mut input_format_context = format::input(&input_path).expect("Failed to open input file");

    for i in 0..input_format_context.nb_streams() {
        let stream = input_format_context.stream(i as usize).unwrap();
        match stream.codec().medium() {
            Type::Video => {
                // Find video stream in the input file
                let video_stream_index = i;
                println!("id:{}", video_stream_index);
                let input_video_stream = stream;
                let mut input_video_decoder = input_video_stream
                    .codec()
                    .decoder()
                    .video()
                    .expect("Failed to find video decoder");

                // Open output URL using FFmpeg
                // let mut output_format_context = format::output(output_url).expect("Failed to open output URL");
                let mut output_format_context =
                    format::output_as(output_url, "flv").expect("Failed to open output URL");

                // Find H.264 video encoder
                let output_codec =
                    codec::encoder::find(codec::Id::H264).expect("Failed to find H.264 encoder");
                let output_video_stream = output_format_context.add_stream(output_codec).unwrap();
                let mut output_video_encoder =
                    output_video_stream.codec().encoder().video().unwrap();

                // Configure output video encoder
                output_video_encoder.set_time_base(input_video_stream.time_base());
                output_video_encoder.set_bit_rate(input_video_decoder.bit_rate());
                output_video_encoder.set_width(input_video_decoder.width());
                output_video_encoder.set_height(input_video_decoder.height());
                output_video_encoder.set_format(Pixel::YUV420P);
                output_video_encoder.set_frame_rate(input_video_decoder.frame_rate());

                let mut output_video_encoder = output_video_encoder.open().unwrap();

                // Write header to output URL
                output_format_context
                    .write_header()
                    .expect("Failed to write output header");

                // Loop through input frames and encode them for output
                let mut frame_number = 0;
                loop {
                    let mut input_packet = Packet::empty();
                    input_packet = match input_packet.read(&mut input_format_context) {
                        Ok(()) => input_packet,
                        Err(_) => break,
                    };
                    // println!("{}{}", input_packet.stream(), video_stream_index as usize);
                    if input_packet.stream() == video_stream_index as usize {
                        let mut input_frame = Video::new(
                            input_video_decoder.format(),
                            input_video_decoder.width(),
                            input_video_decoder.height(),
                        );
                        input_video_decoder
                            .send_packet(&input_packet)
                            .expect("Failed to send packet");
                        loop {
                            match input_video_decoder.receive_frame(&mut input_frame) {
                                Ok(()) => {
                                    let mut output_frame = Video::new(
                                        output_video_encoder.format(),
                                        output_video_encoder.width(),
                                        output_video_encoder.height(),
                                    );
                                    // println!(
                                    //     "iskey:{} iscorrupt:{} pts:{:?} timestamp:{:?} quality:{} flags:{:?} metadata:{:?}",
                                    //     input_frame.is_key(),
                                    //     input_frame.is_corrupt(),
                                    //     input_frame.pts(),
                                    //     input_frame.timestamp(),
                                    //     input_frame.quality(),
                                    //     input_frame.flags(),
                                    //     input_frame.metadata(),
                                    // );
                                    output_frame.set_pts(Some(frame_number));
                                    output_video_encoder
                                        .send_frame(&input_frame)
                                        .expect("Failed to send input_frame");
                                    let mut out_packet = Packet::empty();
                                    match output_video_encoder.receive_packet(&mut out_packet) {
                                        Ok(()) => {
                                            println!("size:{}", out_packet.size());
                                            out_packet
                                                .set_stream(video_stream_index.try_into().unwrap());
                                            out_packet
                                                .write(&mut output_format_context)
                                                .expect("Failed to write output packet ");
                                        }
                                        Err(_) => continue,
                                    };
                                }
                                Err(_) => break,
                            };
                        }
                        frame_number += 1;
                    }
                }

                // Write trailer to output URL
                output_format_context
                    .write_trailer()
                    .expect("Failed to write output trailer");
            }
            Type::Audio => {}
            _ => {}
        }
    }
}
