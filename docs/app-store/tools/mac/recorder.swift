// recorder <window id> <seconds> <out.mov>
// Records one window (even when covered by others) at its full pixel size,
// 30 fps max, H.264, no cursor, no audio. Uses ScreenCaptureKit.
import AppKit
import AVFoundation
import CoreMedia
import Foundation
import ScreenCaptureKit

final class Recorder: NSObject, SCStreamOutput {
    let writer: AVAssetWriter
    let input: AVAssetWriterInput
    var started = false
    var frames = 0
    var firstFrameMs = 0.0

    init(url: URL, width: Int, height: Int) throws {
        try? FileManager.default.removeItem(at: url)
        writer = try AVAssetWriter(outputURL: url, fileType: .mov)
        input = AVAssetWriterInput(mediaType: .video, outputSettings: [
            AVVideoCodecKey: AVVideoCodecType.h264,
            AVVideoWidthKey: width,
            AVVideoHeightKey: height,
            AVVideoCompressionPropertiesKey: [AVVideoAverageBitRateKey: 60_000_000],
        ])
        input.expectsMediaDataInRealTime = true
        writer.add(input)
        writer.startWriting()
    }

    func stream(_ stream: SCStream, didOutputSampleBuffer sb: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .screen, sb.isValid,
              let info = (CMSampleBufferGetSampleAttachmentsArray(sb, createIfNecessary: false) as? [[SCStreamFrameInfo: Any]])?.first,
              let raw = info[.status] as? Int, SCFrameStatus(rawValue: raw) == .complete
        else { return }
        if !started {
            writer.startSession(atSourceTime: sb.presentationTimeStamp)
            started = true
            firstFrameMs = Date().timeIntervalSince1970 * 1000
        }
        if input.isReadyForMoreMediaData, input.append(sb) { frames += 1 }
    }
}

_ = NSApplication.shared // ScreenCaptureKit needs the window server connection
let args = CommandLine.arguments
guard args.count == 4, let windowID = UInt32(args[1]), let seconds = Double(args[2]) else {
    print("usage: recorder <window id> <seconds> <out.mov>"); exit(2)
}
let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: false)
guard let window = content.windows.first(where: { $0.windowID == windowID }) else {
    print("window \(windowID) not found"); exit(1)
}
let filter = SCContentFilter(desktopIndependentWindow: window)
let scale = Double(filter.pointPixelScale)
let config = SCStreamConfiguration()
config.width = Int(window.frame.width * scale)
config.height = Int(window.frame.height * scale)
config.minimumFrameInterval = CMTime(value: 1, timescale: 30)
config.showsCursor = false
config.pixelFormat = kCVPixelFormatType_32BGRA
config.ignoreShadowsSingleWindow = true
let recorder = try Recorder(url: URL(fileURLWithPath: args[3]), width: config.width, height: config.height)
let stream = SCStream(filter: filter, configuration: config, delegate: nil)
try stream.addStreamOutput(recorder, type: .screen, sampleHandlerQueue: DispatchQueue(label: "frames"))
try await stream.startCapture()
print("recording \(config.width)x\(config.height) for \(seconds)s")
try await Task.sleep(nanoseconds: UInt64(seconds * 1_000_000_000))
try await stream.stopCapture()
recorder.input.markAsFinished()
await recorder.writer.finishWriting()
print("firstframe \(Int(recorder.firstFrameMs))")
print("frames \(recorder.frames) → \(args[3]) (\(recorder.writer.status == .completed ? "ok" : "failed: \(String(describing: recorder.writer.error))"))")
