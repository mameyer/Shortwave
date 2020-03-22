// Shortwave - gstreamer_backend.rs
// Copyright (C) 2020  Felix Häcker <haeckerfelix@gnome.org>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use glib::Sender;
use gstreamer::prelude::*;
use gstreamer::{Bin, Element, ElementFactory, GhostPad, Pad, PadProbeId, Pipeline, State};

use std::convert::TryInto;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use crate::app::Action;
use crate::audio::PlaybackState;
use crate::audio::Song;
use crate::settings::{settings_manager, Key};

////////////////////////////////////////////////////////////////////////////////////////////////////
//                                                                                                //
//  # Gstreamer Pipeline                                                                          //
//                                            -----      --------       -------------             //
//                                           |     | -> | queue [1] -> | recorderbin |            //
//    --------------      --------------     |     |     --------       -------------             //
//   | uridecodebin | -> | audioconvert | -> | tee |                                              //
//    --------------      --------------     |     |     -------      -----------                 //
//                                           |     | -> | queue | -> | pulsesink |                //
//                                            -----      -------      -----------                 //
//                                                                                                //
//                                                                                                //
//                                                                                                //
//  We use the the file_srcpad[1] to block the dataflow, so we can change the recorderbin.        //
//  The dataflow gets blocked when the song changes.                                              //
//                                                                                                //
//                                                                                                //
//  But how does recording work in detail?                                                        //
//                                                                                                //
//  1) We start recording a new song, when...                                                     //
//     a) The song title changed, and there's no current recording running                        //
//        [ player.rs -> process_gst_message() -> GstreamerMessage::SongTitleChanged ]            //
//     b) The song title changed, and the old recording stopped                                   //
//        [ player.rs -> process_gst_message() -> GstreamerMessage::RecordingStopped ]            //
//                                                                                                //
//  2) Before we can start recording, we need to ensure that the old recording is stopped.        //
//     This is usually not the case, except it's the first song we record.                        //
//     The recording gets stopped by calling "stop_recording()"                                   //
//     [ player.rs -> process_gst_message() -> GstreamerMessage::SongTitleChanged ]               //
//                                                                                                //
//  3) First of all, we have to make sure the old recorderbin gets destroyed. So we have          //
//     to block the pipeline first at [1], by using a block probe.                                //
//                                                                                                //
//  4) After the pipeline is blocked, we push a EOS event into the recorderbin sinkpad.           //
//     We need the EOS event, otherwise we cannot remove the old recorderbin from the             //
//     running pipeline. Without the EOS event, we would have to stop the whole pipeline.         //
//     With it we can dynamically add/remove recorderbins from the pipeline.                      //
//                                                                                                //
//  5) We detect the EOS event by listening to the pipeline bus. We confirm this by sending       //
//     the "GstreamerMessage::RecordingStopped" message.                                          //
//     [ gstreamer_backend.rs -> parse_bus_message() -> gstreamer::MessageView::Element() ]       //
//                                                                                                //
//  6) After we get this message, we can start recording the new song, by creating a new          //
//     recorderbin with "start_recording()"                                                       //
//     [ player.rs -> process_gst_message() -> GstreamerMessage::RecordingStopped() ]             //
//                                                                                                //
//  7) The recorderbin gets created and appendend to the pipeline. Now the stream gets            //
//     forwarded into a new file again.                                                           //
//                                                                                                //
////////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Clone)]
pub enum GstreamerMessage {
    SongTitleChanged(String),
    PlaybackStateChanged(PlaybackState),
    RecordingStopped,
}

#[allow(dead_code)]
pub struct GstreamerBackend {
    pipeline: Pipeline,

    uridecodebin: Element,
    audioconvert: Element,
    tee: Element,

    audio_queue: Element,
    pulsesink: Element, // TODO: Is it good to hardcode pulsesink here instead of autoaudiosink?

    file_queue: Element,
    recorderbin: Arc<Mutex<Option<RecorderBin>>>,
    file_srcpad: Pad,
    file_blockprobe_id: Option<PadProbeId>,

    current_title: Arc<Mutex<String>>,
    volume: Arc<Mutex<f64>>,
    volume_signal_id: glib::signal::SignalHandlerId,
    sender: Sender<GstreamerMessage>,
}

impl GstreamerBackend {
    pub fn new(gst_sender: Sender<GstreamerMessage>, app_sender: Sender<Action>) -> Self {
        // create gstreamer pipeline
        let pipeline = Pipeline::new(Some("recorder_pipeline"));

        // create pipeline elements
        let uridecodebin = ElementFactory::make("uridecodebin", Some("uridecodebin")).unwrap();
        let audioconvert = ElementFactory::make("audioconvert", Some("audioconvert")).unwrap();
        let tee = ElementFactory::make("tee", Some("tee")).unwrap();
        let audio_queue = ElementFactory::make("queue", Some("audio_queue")).unwrap();
        let pulsesink = ElementFactory::make("pulsesink", Some("pulsesink")).expect("Could not find PulseAudio (Cannot create gstreamer `pulsesink` element).");
        let file_queue = ElementFactory::make("queue", Some("file_queue")).unwrap();
        let file_srcpad = file_queue.get_static_pad("src").unwrap();

        // link pipeline elements
        pipeline.add_many(&[&uridecodebin, &audioconvert, &tee, &audio_queue, &pulsesink, &file_queue]).unwrap();
        Element::link_many(&[&audioconvert, &tee]).unwrap();
        let tee_tempmlate = tee.get_pad_template("src_%u").unwrap();

        // link tee -> queue
        let tee_file_srcpad = tee.request_pad(&tee_tempmlate, None, None).unwrap();
        let _ = tee_file_srcpad.link(&file_queue.get_static_pad("sink").unwrap());

        // link tee -> queue -> pulsesink
        let tee_audio_srcpad = tee.request_pad(&tee_tempmlate, None, None).unwrap();
        let _ = tee_audio_srcpad.link(&audio_queue.get_static_pad("sink").unwrap());
        let _ = audio_queue.link(&pulsesink);

        let recorderbin = Arc::new(Mutex::new(None));

        // dynamically link uridecodebin element with audioconvert element
        uridecodebin.connect_pad_added(clone!(@weak audioconvert => move |_, src_pad| {
            let sink_pad = audioconvert.get_static_pad("sink").expect("Failed to get static sink pad from audioconvert");
            if sink_pad.is_linked() {
                return; // We are already linked. Ignoring.
            }

            let new_pad_caps = src_pad.get_current_caps().expect("Failed to get caps of new pad.");
            let new_pad_struct = new_pad_caps.get_structure(0).expect("Failed to get first structure of caps.");
            let new_pad_type = new_pad_struct.get_name();

            if new_pad_type.starts_with("audio/x-raw") {
                // check if new_pad is audio
                let _ = src_pad.link(&sink_pad);
                return;
            }
        }));

        // Current song title. We need this variable to check if the title have changed.
        let current_title = Arc::new(Mutex::new(String::new()));

        // listen for new pipeline / bus messages
        let bus = pipeline.get_bus().expect("Unable to get pipeline bus");
        bus.add_watch_local(clone!(@strong gst_sender, @weak current_title => @default-panic, move |_, message|{
            Self::parse_bus_message(&message, gst_sender.clone(), current_title);
            Continue(true)
        }))
        .unwrap();

        // We have to update the volume if we get changes from pulseaudio (pulsesink).
        // The user is able to control the volume from g-c-c.
        let volume = Arc::new(Mutex::new(1.0));
        let (volume_sender, volume_receiver) = glib::MainContext::channel(glib::PRIORITY_LOW);

        // We need to do message passing (sender/receiver) here, because gstreamer messages are
        // coming from a other thread (and app::Action enum is not thread safe).
        volume_receiver.attach(
            None,
            clone!(@strong app_sender => move |volume| {
                send!(app_sender, Action::PlaybackSetVolume(volume));
                glib::Continue(true)
            }),
        );

        // Update volume coming from pulseaudio / pulsesink
        let volume_signal_id = pulsesink.connect_notify(
            Some("volume"),
            clone!(@weak volume as old_volume, @strong volume_sender => move |element, _| {
                let new_volume: f64 = element.get_property("volume").unwrap().get().unwrap().unwrap();

                // We have to check if the values are the same. For some reason gstreamer sends us
                // slightly differents floats, so we round up here (only the the first two digits are
                // important for use here).
                let mut old_volume_locked = old_volume.lock().unwrap();
                let new_val = format!("{:.2}", old_volume_locked);
                let old_val = format!("{:.2}", old_volume_locked);

                if new_val != old_val {
                    send!(volume_sender, new_volume);
                    *old_volume_locked = new_volume;
                }
            }),
        );

        // It's possible to mute the audio (!= 0.0) from pulseaudio side, so we should handle
        // this too by setting the volume to 0.0
        pulsesink.connect_notify(
            Some("mute"),
            clone!(@weak volume as old_volume, @strong volume_sender => move |element, _| {
                let mute: bool = element.get_property("mute").unwrap().get().unwrap().unwrap();
                let mut old_volume_locked = old_volume.lock().unwrap();
                if mute && *old_volume_locked != 0.0 {
                    send!(volume_sender, 0.0);
                    *old_volume_locked = 0.0;
                }
            }),
        );

        Self {
            pipeline,
            uridecodebin,
            audioconvert,
            tee,
            audio_queue,
            pulsesink,
            file_queue,
            recorderbin,
            file_srcpad,
            file_blockprobe_id: None,
            current_title,
            volume,
            volume_signal_id,
            sender: gst_sender,
        }
    }

    pub fn set_state(&mut self, state: gstreamer::State) {
        if state == gstreamer::State::Null {
            send!(self.sender, GstreamerMessage::PlaybackStateChanged(PlaybackState::Stopped));
        }

        let _ = self.pipeline.set_state(state);
    }

    pub fn set_volume(&self, volume: f64) {
        // We need to block the signal, otherwise we risk creating a endless loop
        glib::signal::signal_handler_block(&self.pulsesink, &self.volume_signal_id);
        *self.volume.lock().unwrap() = volume;
        self.pulsesink.set_property("volume", &volume).unwrap();
        glib::signal::signal_handler_unblock(&self.pulsesink, &self.volume_signal_id);
    }

    pub fn new_source_uri(&mut self, source: &str) {
        debug!("Stop pipeline...");
        let _ = self.pipeline.set_state(State::Null);

        debug!("Set new source URI...");
        self.uridecodebin.set_property("uri", &source).unwrap();

        debug!("Start pipeline...");
        let _ = self.pipeline.set_state(State::Playing);
    }

    pub fn start_recording(&mut self, path: PathBuf) {
        debug!("Start recording to {:?}", path);

        // We need to set an offset, otherwise the length of the recorded song would be wrong.
        // Get current clock time and calculate offset
        let clock = self.pipeline.get_clock().expect("Could not get gstreamer pipeline clock");
        debug!("( Clock time: {} )", clock.get_time());
        let offset = -(clock.get_time().nseconds().unwrap() as i64);
        self.file_srcpad.set_offset(offset);

        let mut recorderbin_locked = self.recorderbin.lock().unwrap();
        if let Some(x) = &*recorderbin_locked {
            x.destroy();
        } else {
            debug!("No old recorderbin available - nothing to destroy.");
        }

        debug!("Create new recorderbin.");
        let recorderbin = RecorderBin::new(self.get_current_song_title(), path, self.pipeline.clone(), &self.file_srcpad);
        *recorderbin_locked = Some(recorderbin);

        // Remove block probe id, if available
        match self.file_blockprobe_id.take() {
            Some(id) => {
                self.file_srcpad.remove_probe(id);
                debug!("Removed block probe.");
            }
            None => debug!("No block probe to remove."),
        }
    }

    pub fn stop_recording(&mut self, save_song: bool) -> Option<Song> {
        let recorderbin = self.recorderbin.lock().unwrap().clone();

        // Check if recorderbin is available
        if let Some(recorderbin) = recorderbin {
            // Check if we want to save the recorded data
            // Sometimes we can discard it as is got interrupted / not completely recorded
            if save_song {
                // Add a block probe to the file source pad to block the data flow in the pipeline
                let rbin = recorderbin.clone();
                let file_id = self.file_srcpad.add_probe(gstreamer::PadProbeType::BLOCK_DOWNSTREAM, move |_, _| {
                    // Dataflow is blocked
                    debug!("Push EOS into recorderbin sinkpad...");
                    if let Some(sinkpad) = rbin.gstbin.get_static_pad("sink") {
                        sinkpad.send_event(gstreamer::Event::new_eos().build());
                    }
                    gstreamer::PadProbeReturn::Ok
                });

                // We need the padprobe id later to remove the block probe
                self.file_blockprobe_id = file_id;

                // Create song and return it
                let song = recorderbin.stop();

                // Check song duration
                // Few stations are using the song metadata field as newsticker,
                // which means the text changes every few seconds.
                // Because of this reason, we shouldn't record songs with a too low duration.
                let threshold: u64 = settings_manager::get_integer(Key::RecorderSongDurationThreshold).try_into().unwrap();
                if song.duration > std::time::Duration::from_secs(threshold) {
                    Some(song)
                } else {
                    info!("Ignore song \"{}\". Duration is not long enough.", song.title);
                    None
                }
            } else {
                // Discard recorded data
                debug!("Discard recorded data.");
                if let Err(err) = fs::remove_file(&recorderbin.song_path) {
                    warn!("Could not delete recorded data: {}", err);
                }
                recorderbin.destroy();

                // Recorderbin got destroyed, so make out of the Option<RecorderBin> a None!
                self.recorderbin.lock().unwrap().take().unwrap();

                None
            }
        } else {
            debug!("No recorderbin available - nothing to stop.");
            None
        }
    }

    pub fn is_recording(&self) -> bool {
        self.recorderbin.lock().unwrap().is_some()
    }

    pub fn get_current_song_title(&self) -> String {
        self.current_title.lock().unwrap().clone()
    }

    fn parse_bus_message(message: &gstreamer::Message, sender: Sender<GstreamerMessage>, current_title: Arc<Mutex<String>>) {
        match message.view() {
            gstreamer::MessageView::Tag(tag) => {
                if let Some(t) = tag.get_tags().get::<gstreamer::tags::Title>() {
                    let new_title = t.get().unwrap().to_string();

                    // only send message if song title really have changed.
                    let mut current_title_locked = current_title.lock().unwrap();
                    if *current_title_locked != new_title {
                        *current_title_locked = new_title.clone();
                        send!(sender, GstreamerMessage::SongTitleChanged(new_title));
                    }
                }
            }
            gstreamer::MessageView::StateChanged(sc) => {
                let playback_state = match sc.get_current() {
                    gstreamer::State::Playing => PlaybackState::Playing,
                    gstreamer::State::Paused => PlaybackState::Playing,
                    gstreamer::State::Ready => PlaybackState::Playing,
                    _ => PlaybackState::Stopped,
                };

                send!(sender, GstreamerMessage::PlaybackStateChanged(playback_state));
            }
            gstreamer::MessageView::Element(element) => {
                let structure = element.get_structure().unwrap();
                if structure.get_name() == "GstBinForwarded" {
                    let message: gstreamer::message::Message = structure.get("message").unwrap().unwrap();
                    if let gstreamer::MessageView::Eos(_) = &message.view() {
                        // recorderbin got EOS which means the current song got successfully saved.
                        debug!("Recorderbin received EOS event.");

                        send!(sender, GstreamerMessage::RecordingStopped);
                    }
                }
            }
            gstreamer::MessageView::Error(err) => {
                let msg = err.get_error().to_string();
                warn!("Gstreamer Error: {:?}", msg);
                send!(sender, GstreamerMessage::PlaybackStateChanged(PlaybackState::Failure(msg)));
            }
            _ => (),
        };
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////
//                                                                                                      //
//  # RecorderBin                                                                                       //
//                                                                                                      //
//    --------------------------------------------------------------                                    //
//   |                  -----------       --------      ----------  |                                   //
//   | ( ghostpad ) -> | vorbisenc | ->  | oggmux | -> | filesink | |                                   //
//   |                  -----------       --------      ----------  |                                   //
//    --------------------------------------------------------------                                    //
//                                                                                                      //
/////////////////////////////////////////////////////////////////////////////////////////////////////////

#[allow(dead_code)]
#[derive(Clone)]
struct RecorderBin {
    pub gstbin: Bin,
    pipeline: Pipeline,

    ghostpad: GhostPad,
    vorbisenc: Element,
    oggmux: Element,
    filesink: Element,

    song_title: String,
    pub song_path: PathBuf,
    song_timestamp: SystemTime,
}

impl RecorderBin {
    pub fn new(song_title: String, song_path: PathBuf, pipeline: Pipeline, srcpad: &Pad) -> Self {
        // Create elements
        let vorbisenc = ElementFactory::make("vorbisenc", Some("vorbisenc")).unwrap();
        let oggmux = ElementFactory::make("oggmux", Some("oggmux")).unwrap();
        let filesink = ElementFactory::make("filesink", Some("filesink")).unwrap();
        filesink.set_property("location", &song_path.to_str().unwrap()).unwrap();

        // Create bin itself
        let bin = Bin::new(Some("bin"));
        bin.set_property("message-forward", &true).unwrap();

        // Add elements to bin and link them
        bin.add(&vorbisenc).unwrap();
        bin.add(&oggmux).unwrap();
        bin.add(&filesink).unwrap();
        Element::link_many(&[&vorbisenc, &oggmux, &filesink]).unwrap();

        // Add bin to pipeline
        pipeline.add(&bin).expect("Could not add recorderbin to pipeline");

        // Link file_srcpad with vorbisenc sinkpad using a ghostpad
        let vorbisenc_sinkpad = vorbisenc.get_static_pad("sink").unwrap();
        let ghostpad = gstreamer::GhostPad::new(Some("sink"), &vorbisenc_sinkpad).unwrap();
        bin.add_pad(&ghostpad).unwrap();
        bin.sync_state_with_parent().expect("Unable to sync recorderbin state with pipeline");
        srcpad.link(&ghostpad).expect("Queue src pad cannot linked to vorbisenc sinkpad");

        // Set song timestamp so we can check the duration later
        let song_timestamp = SystemTime::now();

        Self {
            gstbin: bin,
            pipeline,
            ghostpad,
            vorbisenc,
            oggmux,
            filesink,
            song_title,
            song_path,
            song_timestamp,
        }
    }

    pub fn stop(&self) -> Song {
        let now = SystemTime::now();
        let duration = now.duration_since(self.song_timestamp).unwrap();

        Song::new(&self.song_title, self.song_path.clone(), duration)
    }

    pub fn destroy(&self) {
        match self.pipeline.remove(&self.gstbin) {
            Ok(_) => (),
            Err(_) => warn!("Could not remove recorderbin from pipeline."),
        }
        self.gstbin.set_state(State::Null).unwrap();
        debug!("Destroyed recorderbin.");
    }
}
