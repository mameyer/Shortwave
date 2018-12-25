use gstreamer::prelude::*;
use gstreamer_pbutils::prelude::*;

/////////////////////////////////////////////////////////////////////////////////////////////
//                                                                                         //
//  # Gstreamer Pipeline #                                                                 //
//    --------------      --------------      -----------      -------      ------------   //
//   | uridecodebin | -> | audioconvert | -> | vorbisenc | -> | queue | -> | muxsinkbin |  //
//    --------------      --------------      -----------      -------      ------------   //
//                                                                                         //
//  # muxsinkbin:  (gstreamer::Bin) #                                                      //
//    --------------------------------------------                                         //
//   |                  --------      ----------  |                                        //
//   | ( ghostpad ) -> | oggmux | -> | filesink | |                                        //
//   |                  --------      ----------  |                                        //
//    --------------------------------------------                                         //
//                                                                                         //
/////////////////////////////////////////////////////////////////////////////////////////////

pub struct RecorderBackend{
    pub pipeline: gstreamer::Pipeline,

    pub uridecodebin: gstreamer::Element,
    pub audioconvert: gstreamer::Element,
    pub vorbisenc: gstreamer::Element,
    pub queue: gstreamer::Element,
    pub muxsinkbin: Option<gstreamer::Bin>,

    pub queue_srcpad: gstreamer::Pad,
    pub queue_blockprobe_id: Option<gstreamer::PadProbeId>,
}

impl RecorderBackend{
    pub fn new() -> Self{
        // create gstreamer pipeline
        let pipeline = gstreamer::Pipeline::new("recorder_pipeline");

        // create pipeline elements
        let uridecodebin = gstreamer::ElementFactory::make("uridecodebin", "uridecodebin").unwrap();
        let audioconvert = gstreamer::ElementFactory::make("audioconvert", "audioconvert").unwrap();
        let vorbisenc = gstreamer::ElementFactory::make("vorbisenc", "vorbisenc").unwrap();
        let queue = gstreamer::ElementFactory::make("queue", "queue").unwrap();

        // link pipeline elements
        pipeline.add_many(&[&uridecodebin, &audioconvert, &vorbisenc, &queue]).unwrap();
        gstreamer::Element::link_many(&[&audioconvert, &vorbisenc, &queue]).unwrap();

        // dynamically link uridecodebin element with audioconvert element
        let convert = audioconvert.clone();
        uridecodebin.connect_pad_added(move |uridecodebin, src_pad|{
            let sink_pad = convert.get_static_pad("sink").expect("Failed to get static sink pad from convert");
            if sink_pad.is_linked() {
                return; // We are already linked. Ignoring.
            }

            let new_pad_caps = src_pad.get_current_caps().expect("Failed to get caps of new pad.");
            let new_pad_struct = new_pad_caps.get_structure(0).expect("Failed to get first structure of caps.");
            let new_pad_type = new_pad_struct.get_name();

            if new_pad_type.starts_with("audio/x-raw") { // check if new_pad is audio
                let _ = src_pad.link(&sink_pad);
                return;
            }
        });

        let queue_srcpad = queue.get_static_pad("src").unwrap();

        let mut pipeline = Self{
            pipeline,
            uridecodebin,
            audioconvert,
            vorbisenc,
            queue,
            muxsinkbin: None,
            queue_srcpad,
            queue_blockprobe_id: None,
        };

        pipeline.create_muxsinkbin("/dev/null");
        pipeline
    }

    pub fn new_source_uri(&mut self, source: &str){
        debug!("Stop pipeline...");
        let _ = self.pipeline.set_state(gstreamer::State::Null);

        debug!("Set new source uri...");
        self.uridecodebin.set_property("uri", &source).unwrap();

        debug!("Start pipeline...");
        let _ = self.pipeline.set_state(gstreamer::State::Playing);
    }

    pub fn new_filesink_location(&mut self, location: &str){
        debug!("Update filesink location to \"{}\"...", location);

        debug!("Destroy old muxsinkbin");
        let muxsinkbin = self.muxsinkbin.take().unwrap();
        muxsinkbin.set_state(gstreamer::State::Null);
        self.pipeline.remove(&muxsinkbin);

        debug!("Create new muxsinkbin");
        self.create_muxsinkbin(location);

        debug!("Remove block probe...");
        self.queue_srcpad.remove_probe(self.queue_blockprobe_id.take().unwrap());
    }

    fn create_muxsinkbin(&mut self, location: &str){
        // Create oggmux
        let oggmux = gstreamer::ElementFactory::make("oggmux", "oggmux").unwrap();

        // Create filesink
        let filesink = gstreamer::ElementFactory::make("filesink", "filesink").unwrap();
        filesink.set_property("location", &location).unwrap();

        // Create bin
        let bin = gstreamer::Bin::new("bin");
        bin.set_property("message-forward", &true).unwrap();

        // Add elements to bin and link them
        bin.add(&oggmux).unwrap();
        bin.add(&filesink).unwrap();
        gstreamer::Element::link_many(&[&oggmux, &filesink]).unwrap();

        // Add bin to pipeline
        self.pipeline.add(&bin).unwrap();

        // Link queue src pad with oggmux sinkpad using a ghostpad
        let sinkpad_template = oggmux.get_pad_template("audio_%u").unwrap();
        let oggmux_sinkpad = oggmux.request_pad(&sinkpad_template, None, None).unwrap();

        let ghostpad = gstreamer::GhostPad::new("sink", &oggmux_sinkpad).unwrap();
        bin.add_pad(&ghostpad).unwrap();
        bin.sync_state_with_parent();

        if self.queue_srcpad.link(&ghostpad) != gstreamer::PadLinkReturn::Ok {
            warn!("Queue src pad cannot linked to oggmux sinkpad");
        }

        self.muxsinkbin = Some(bin);
    }
}
