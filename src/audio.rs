use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    BufferSize, Device, Sample, SampleFormat, Stream, StreamConfig,
};

pub struct OutputStreamParams {
    output_device: Device,
    stream_config: StreamConfig,
    sample_format: SampleFormat,
}

impl Default for OutputStreamParams {
    fn default() -> Self {
        let output_device = cpal::default_host().default_output_device().unwrap();
        let default_config = output_device.default_output_config().unwrap();

        Self {
            output_device,
            stream_config: StreamConfig {
                channels: 2,
                sample_rate: default_config.sample_rate(),
                buffer_size: BufferSize::Default,
            },
            sample_format: default_config.sample_format(),
        }
    }
}

struct AudioRenderer {}

impl AudioRenderer {
    fn new() -> Self {
        Self {}
    }

    fn render_audio<S: Sample>(&mut self, buffer: &mut [S]) {
        for s in buffer.iter_mut() {
            *s = S::from::<f32>(&0.0);
        }
        todo!()
    }
}

pub struct AudioOut {
    renderer: AudioRenderer,
}

impl AudioOut {
    fn start_stream(self, output_stream_params: OutputStreamParams) -> Stream {
        let OutputStreamParams {
            output_device,
            stream_config,
            sample_format,
        } = output_stream_params;

        let stream = match sample_format {
            SampleFormat::I16 => panic!("I16 sample format not supported"),
            SampleFormat::U16 => panic!("U16 sample format not supported"),
            SampleFormat::F32 => self.create_stream::<f32>(&output_device, &stream_config),
        };
        stream.play().unwrap();
        stream
    }

    fn create_stream<S: Sample>(mut self, device: &Device, config: &StreamConfig) -> Stream {
        device
            .build_output_stream(
                config,
                move |buffer: &mut [S], _| {
                    self.renderer.render_audio(buffer);
                },
                |err| eprintln!("{}", err),
            )
            .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time::Duration};
    #[test]
    fn default_channel_is_2() {
        let audio = OutputStreamParams::default();
        assert_eq!(audio.stream_config.channels, 2);
    }

    #[test]
    fn audio_out_start_stream() {
        let audio_out = AudioOut {
            renderer: AudioRenderer::new(),
        };
        let stream = audio_out.start_stream(OutputStreamParams::default());
        thread::sleep(Duration::from_millis(100));
        stream.pause().unwrap();
    }
}
