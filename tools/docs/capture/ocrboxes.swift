// Prints every line of text Vision finds in an image with its box, so a driver can click text that is
// not in the accessibility tree.
// Usage: ocrboxes IMAGE        Output: X<TAB>Y<TAB>W<TAB>H<TAB>TEXT  (pixels, origin top left)
import AppKit
import Foundation
import Vision

guard CommandLine.arguments.count == 2,
      let image = NSImage(contentsOfFile: CommandLine.arguments[1]),
      let cg = image.cgImage(forProposedRect: nil, context: nil, hints: nil) else {
    FileHandle.standardError.write(Data("usage: ocrboxes IMAGE\n".utf8))
    exit(2)
}
let request = VNRecognizeTextRequest()
request.recognitionLevel = .accurate
request.usesLanguageCorrection = false
try VNImageRequestHandler(cgImage: cg).perform([request])
let width = Double(cg.width), height = Double(cg.height)
for observation in request.results ?? [] {
    guard let text = observation.topCandidates(1).first?.string else { continue }
    let box = observation.boundingBox
    print("\(Int(box.minX * width))\t\(Int((1 - box.maxY) * height))\t\(Int(box.width * width))\t\(Int(box.height * height))\t\(text)")
}
