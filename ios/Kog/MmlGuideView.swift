import SwiftUI

#if KOG_NATIVE_AUDIO
@_silgen_name("kog_mml_guide")
private func nativeGuide() -> UnsafeMutablePointer<CChar>?
@_silgen_name("kog_audio_string_free")
private func freeGuide(_ string: UnsafeMutablePointer<CChar>)
#endif

/// One chapter of the Kog MML guide (docs/mml-guide).
struct GuideChapter: Decodable, Identifiable {
    let title: String
    let markdown: String
    var id: String { title }

    static func load() -> [GuideChapter] {
        #if KOG_NATIVE_AUDIO
        guard let json = nativeGuide() else { return [] }
        defer { freeGuide(json) }
        return (try? JSONDecoder().decode([GuideChapter].self, from: Data(String(cString: json).utf8))) ?? []
        #else
        return []
        #endif
    }
}

/// The guide as a book: a chapter list that opens each chapter's text.
struct MmlGuideView: View {
    @Environment(\.dismiss) private var dismiss
    private let chapters = GuideChapter.load()

    var body: some View {
        NavigationStack {
            List(Array(chapters.enumerated()), id: \.element.id) { index, chapter in
                NavigationLink(chapter.title) { GuideChapterView(chapters: chapters, index: index) }
            }
            .overlay { if chapters.isEmpty { Text("The guide is unavailable in this build.").foregroundStyle(Palette.muted) } }
            .navigationTitle("Kog MML Guide").navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .topBarTrailing) { Button("Done") { dismiss() } } }
        }
    }
}

private struct GuideChapterView: View {
    let chapters: [GuideChapter]
    @State var index: Int

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 10) {
                    ForEach(Array(GuideBlock.parse(chapters[index].markdown).enumerated()), id: \.offset) { _, block in
                        block.view
                    }
                }
                .padding()
                .id("top")
            }
            .onChange(of: index) { _, _ in proxy.scrollTo("top", anchor: .top) }
        }
        .background(Palette.window)
        .navigationTitle(chapters[index].title).navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItemGroup(placement: .bottomBar) {
                Button("‹ Previous") { index -= 1 }.disabled(index == 0)
                Spacer()
                Text("\(index + 1) of \(chapters.count)").font(.caption).foregroundStyle(Palette.muted)
                Spacer()
                Button("Next ›") { index += 1 }.disabled(index + 1 == chapters.count)
            }
        }
    }
}

/// A heading, paragraph or list item, or preformatted text (code or table).
enum GuideBlock {
    case heading(String, Int)
    case paragraph(String)
    case preformatted(String)

    @ViewBuilder var view: some View {
        switch self {
        case let .heading(text, level):
            Text(text).font(level == 1 ? .title2.bold() : .headline).padding(.top, 6)
        case let .paragraph(text):
            Text((try? AttributedString(markdown: text)) ?? AttributedString(text)).font(.body)
        case let .preformatted(text):
            ScrollView(.horizontal) {
                Text(text).font(.system(.caption, design: .monospaced)).padding(8)
            }
            .background(Color.black.opacity(0.35)).clipShape(RoundedRectangle(cornerRadius: 6))
        }
    }

    static func parse(_ markdown: String) -> [GuideBlock] {
        var blocks = [GuideBlock]()
        var paragraph = ""
        func flush() { if !paragraph.isEmpty { blocks.append(.paragraph(paragraph)); paragraph = "" } }
        let lines = markdown.components(separatedBy: "\n")
        var i = 0
        while i < lines.count {
            let line = lines[i], trimmed = line.trimmingCharacters(in: .whitespaces)
            if trimmed.hasPrefix("```") {
                flush(); i += 1
                var code = [String]()
                while i < lines.count && !lines[i].trimmingCharacters(in: .whitespaces).hasPrefix("```") { code.append(lines[i]); i += 1 }
                blocks.append(.preformatted(code.joined(separator: "\n")))
            } else if trimmed.hasPrefix("#") {
                flush()
                let level = trimmed.prefix { $0 == "#" }.count
                blocks.append(.heading(String(trimmed.dropFirst(level)).trimmingCharacters(in: .whitespaces), level))
            } else if trimmed.hasPrefix("|") {
                flush()
                var table = [String]()
                while i < lines.count && lines[i].trimmingCharacters(in: .whitespaces).hasPrefix("|") {
                    let row = lines[i].trimmingCharacters(in: .whitespaces)
                    if !row.hasPrefix("| ---") { table.append(row.replacingOccurrences(of: "\\|", with: "|").replacingOccurrences(of: "`", with: "")) }
                    i += 1
                }
                blocks.append(.preformatted(table.joined(separator: "\n")))
                continue
            } else if trimmed.isEmpty {
                flush()
            } else if line == trimmed && (trimmed.hasPrefix("- ") || trimmed.range(of: #"^\d+\. "#, options: .regularExpression) != nil) {
                flush()
                paragraph = trimmed.hasPrefix("- ") ? "• " + trimmed.dropFirst(2) : trimmed
            } else {
                paragraph += (paragraph.isEmpty ? "" : " ") + trimmed
            }
            i += 1
        }
        flush()
        return blocks
    }
}
