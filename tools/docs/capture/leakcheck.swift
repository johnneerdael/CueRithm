// Reads the text in screenshots (Vision OCR) and prints every line that could identify the owner:
// a user name or home folder, an e-mail address, a web address (a configured service URL is private), or a
// credential or licence word.
// Usage: leakcheck [--all] IMAGE...      (--all prints every recognised line instead)
// Output: IMAGE<TAB>REASON<TAB>TEXT. No output for an image means nothing matched, which is a
// second check on top of looking at the image, not a replacement for it.
import AppKit
import Foundation
import Vision

let rules: [(String, String)] = [
    ("user name", "(?i)jneerdael|neerdael|john"),
    ("home folder", "(?i)/users/|/home/|c:\\\\users"),
    ("e-mail", "[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\\.[A-Za-z]{2,}"),
    ("credential or licence", "(?i)licen[cs]e|serial|passw|secret|api[ _-]?key|bearer|token"),
    ("web address", "(?i)https?://(?!localhost)|\\b[a-z0-9-]+\\.(com|net|org|io|nl|es|dev|app|cloud)\\b"),
]
var args = Array(CommandLine.arguments.dropFirst())
let all = args.first == "--all"
if all { args.removeFirst() }
var found = false
for path in args {
    guard let image = NSImage(contentsOfFile: path),
          let cg = image.cgImage(forProposedRect: nil, context: nil, hints: nil) else {
        print("\(path)\tUNREADABLE\t")
        found = true
        continue
    }
    let request = VNRecognizeTextRequest()
    request.recognitionLevel = .accurate
    request.usesLanguageCorrection = false
    do { try VNImageRequestHandler(cgImage: cg).perform([request]) } catch {
        print("\(path)\tOCR FAILED\t\(error)")
        found = true
        continue
    }
    for observation in request.results ?? [] {
        guard let text = observation.topCandidates(1).first?.string else { continue }
        if all { print("\(path)\t\t\(text)"); continue }
        for (reason, pattern) in rules where text.range(of: pattern, options: .regularExpression) != nil {
            print("\(path)\t\(reason)\t\(text)")
            found = true
        }
    }
}
exit(found && !all ? 1 : 0)
