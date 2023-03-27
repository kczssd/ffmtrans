use std::ops::Deref;

// crate
struct Frame;
impl Frame {
    fn empty() -> Self {
        Frame
    }
}
struct Video(Frame);
impl Video {
    fn empty() -> Self {
        Video(Frame::empty())
    }
}
impl Deref for Video {
    type Target = Frame;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
struct Audio(Frame);
impl Audio {
    fn empty() -> Self {
        Audio(Frame::empty())
    }
}
impl Deref for Audio {
    type Target = Frame;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
// user
struct Media<T: Deref<Target = Frame>> {
    m: Option<T>,
}
impl<T: Deref<Target = Frame>> Media<T> {
    fn test_1(&mut self, t: T) -> Self {
        Media { m: Some(t) }
    }
    fn test_2(&mut self) -> Media<Video> {
        // something
        Media {
            m: Some(Video::empty()),
        }
    }
}

fn main() {
    let media = Media {
        m: Some(Video::empty()),
    };
    let media2 = Media {
        m: Some(Audio::empty()),
    };
}
