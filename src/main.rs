use ffmpeg_next::codec::decoder;
use ffmpeg_next::format::context::Input as CtInput;
use ffmpeg_next::format::Input;
use ffmpeg_next::media::Type;
use ffmpeg_next::{codec, dictionary, format, Error};
use ffmpeg_next::{Format, Frame, Packet};
use ffmpeg_sys_next::AVInputFormat;
use std::ptr;

unsafe fn open_with_local(path: &str) {
    let ptr: *mut AVInputFormat = ptr::null_mut();
    let open_format = Format::Input(Input::wrap(ptr));
    // 封装器上下文
    let mut input = format::open(&path, &open_format)
        .expect("open error")
        .input();
    // decoder
    let mut video_decoder: Option<decoder::Video> = None;
    let mut audio_decoder: Option<decoder::Audio> = None;
    // stream_id
    let mut vid_stream: usize = 0;
    let mut aud_stream: usize = 0;
    for i in 0..input.nb_streams() {
        let stream = input.stream(i as usize).expect("can't find stream");
        let media_type = stream.codec().medium();
        match media_type {
            Type::Video => {
                video_decoder = Some(
                    stream
                        .codec()
                        .decoder()
                        .video()
                        .expect("get video information failed"),
                );
                vid_stream = i as usize;
            }
            Type::Audio => {
                audio_decoder = Some(
                    stream
                        .codec()
                        .decoder()
                        .audio()
                        .expect("get audio information failed"),
                );
                aud_stream = i as usize;
            }
            _ => {
                println!("unhandled media_type {:?}", media_type);
            }
        }
    }
    if video_decoder.is_some() && audio_decoder.is_some() {
        read_frame(
            &mut input,
            &mut video_decoder.unwrap(),
            &mut audio_decoder.unwrap(),
            vid_stream,
            aud_stream,
        );
    }
}

unsafe fn read_frame(
    mut input: &mut CtInput,
    video_decoder: &mut decoder::Video,
    audio_decoder: &mut decoder::Audio,
    vid_stream: usize,
    aud_stream: usize,
) {
    loop {
        let mut packet = Packet::empty();
        let res = packet.read(&mut input);
        if res.is_err() {
            break;
        }
        if packet.stream() == vid_stream {
            let mut frame = Frame::empty();
            decode_video_frame(video_decoder, &packet, frame).expect("decode video frame failed");
        }
        // else if packet.stream() == aud_stream {
        //     let mut frame = Frame::empty();
        //     decode_audio_frame(audio_decoder, &packet, &mut frame)
        //         .expect("decode video frame failed");
        //     enqueue(frame);
        // }
    }
}
unsafe fn decode_video_frame(
    decoder: &mut decoder::Video,
    packet: &Packet,
    mut frame: Frame,
) -> Result<(), Error> {
    decoder
        .send_packet(packet)
        .expect("send video frame failed");
    loop {
        let res = decoder.receive_frame(&mut frame);
        if res.is_err() {
            break;
        } else {
            println!("frame is empty? {}", frame.is_empty());
            enqueue(&frame);
        }
    }
    Ok(())
}
fn decode_audio_frame(
    decoder: &mut decoder::Audio,
    packet: &Packet,
    frame: &mut Frame,
) -> Result<(), Error> {
    decoder
        .send_packet(packet)
        .expect("send audio frame failed");
    loop {
        let res = decoder.receive_frame(frame);
        if res.is_err() {
            break;
        }
    }
    Ok(())
}
fn enqueue(frame: &Frame) {
    // TODO:enqueue
    println!(
        "iskey:{} iscorrupt:{} pts:{:?} timestamp:{:?} quality:{} flags:{:?} metadata:{:?}",
        frame.is_key(),
        frame.is_corrupt(),
        frame.pts(),
        frame.timestamp(),
        frame.quality(),
        frame.flags(),
        frame.metadata(),
    )
}

unsafe fn open_with_web(path: &str) {
    format::network::init();
    let ptr: *mut AVInputFormat = ptr::null_mut();
    let open_format = Format::Input(Input::wrap(ptr));
    let mut options = dictionary::Owned::new();
    options.set("rtsp_transport", "tcp");
    options.set("max_delay", "550");
    let context = format::open_with(&path, &open_format, options)
        .expect("open error")
        .input();
    // video information
    let duration: f32 = context.duration() as f32 / 1000000.0;
    println!("duration:{duration}s");
    for meta in context.metadata().iter() {
        println!("{} {}", meta.0, meta.1);
    }
}

fn main() {
    // println!("{}", codec::configuration());
    unsafe {
        // format::register_all();
        open_with_local("testmv.mp4");
        // open_with_web("rtsp://localhost:8554/stream");
    }
}
