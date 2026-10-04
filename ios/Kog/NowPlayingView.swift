import SwiftUI
import MediaPlayer

struct NowPlayingView: View {
    @EnvironmentObject private var store: KogStore
    @Environment(\.dismiss) private var dismiss
    @State private var showVisualizer = false
    @State private var spectrum = false
    @State private var scrubbing = false
    @State private var scrubPosition = 0.0
    init(initialVisualization: Bool = false) { _showVisualizer = State(initialValue: initialVisualization) }
    var body: some View {
        ScrollView {
            VStack(spacing: 20) {
                HStack {
                    Button { dismiss() } label: { Image(systemName: "chevron.down").frame(width: 44, height: 44) }.accessibilityLabel("Close player")
                    Spacer(); Text("NOW PLAYING").font(.caption.bold()).foregroundStyle(Palette.muted)
                    Spacer(); Button { showVisualizer.toggle() } label: { Image(systemName: "waveform.path").frame(width: 44, height: 44) }.accessibilityLabel("Show audio visualizer")
                }
                if showVisualizer {
                    WaveformView(audio: store.visualization, spectrum: spectrum, playing: store.playing)
                        .frame(height: 240).background(Palette.panel, in: RoundedRectangle(cornerRadius: 16))
                    Picker("Visualizer", selection: $spectrum) { Text("Waveform").tag(false); Text("Spectrum").tag(true) }.pickerStyle(.segmented)
                } else if let track = store.current {
                    KogArtwork(track: track).frame(maxWidth: 330).aspectRatio(1, contentMode: .fit)
                        .background(Palette.raised).clipShape(RoundedRectangle(cornerRadius: 16))
                }
                VStack(spacing: 6) {
                    Text(store.current?.label ?? "Ready to play").font(.title2.bold()).multilineTextAlignment(.center)
                    Text(store.current?.detail ?? "").foregroundStyle(Palette.muted).multilineTextAlignment(.center)
                }
                VStack(spacing: 0) {
                    Slider(value: Binding(get: { min(scrubbing ? scrubPosition : store.position, max(store.duration, 0.01)) }, set: { scrubPosition = $0 }), in: 0...max(store.duration, 0.01)) { editing in
                        if editing { scrubPosition = store.position; scrubbing = true }
                        else { store.seek(scrubPosition); scrubbing = false }
                    }.disabled(store.duration <= 0).accessibilityLabel("Playback position")
                    HStack { Text(time(scrubbing ? scrubPosition : store.position)); Spacer(); Text(time(store.duration)) }
                        .font(.caption.monospacedDigit()).foregroundStyle(Palette.muted)
                }
                HStack(spacing: 0) {
                    control("shuffle", "Shuffle: \(store.shuffle.label)", active: store.shuffle != .off) { store.cycleShuffle() }
                    Spacer(minLength: 0)
                    control("backward.end.fill", "Previous") { store.previous() }
                    Spacer(minLength: 0)
                    Button { store.togglePlayback() } label: {
                        Image(systemName: store.playing ? "pause.fill" : "play.fill").font(.system(size: 26))
                            .foregroundStyle(Palette.window).frame(width: 64, height: 64).background(.white, in: Circle())
                    }.accessibilityLabel(store.playing ? "Pause" : "Play")
                    Spacer(minLength: 0)
                    control("forward.end.fill", "Next") { store.next() }
                    Spacer(minLength: 0)
                    control(store.repeatMode == .one ? "repeat.1" : "repeat", "Repeat: \(store.repeatMode.label)", active: store.repeatMode != .off) {
                        store.cycleRepeat()
                    }
                }
                HStack {
                    control("stop.fill", "Stop") { store.stop() }
                    Spacer()
                    Button { Task { await store.toggleRadio() } } label: {
                        Label("Radio", systemImage: "die.face.5.fill").foregroundStyle(store.radio ? Palette.accent : Palette.muted)
                    }.frame(minHeight: 44).accessibilityValue(store.radio ? "On" : "Off")
                    Spacer()
                    if let track = store.current {
                        control(store.isStarred(track) ? "star.fill" : "star", "Star", active: store.isStarred(track)) { Task { await store.toggleStar(track) } }
                    }
                }
                HStack {
                    control(store.muted ? "speaker.slash.fill" : "speaker.wave.2.fill", store.muted ? "Unmute" : "Mute") { store.muted.toggle() }
                    Slider(value: $store.volume, in: 0...1).accessibilityLabel("Kog volume")
                    Text("\(Int(store.volume * 100))%").font(.caption.monospacedDigit()).frame(width: 40)
                }
            }.padding(.horizontal, 20).padding(.bottom, 24).frame(maxWidth: 540).frame(maxWidth: .infinity)
        }.background(Palette.window.ignoresSafeArea()).presentationDragIndicator(.visible)
    }
    private func control(_ symbol: String, _ label: String, active: Bool = false, action: @escaping () -> Void) -> some View {
        Button(action: action) { Image(systemName: symbol).font(.system(size: 21)).foregroundStyle(active ? Palette.accent : .white).frame(width: 44, height: 48) }.accessibilityLabel(label)
    }
    private func time(_ seconds: Double) -> String {
        let value = seconds.isFinite ? max(0, Int(seconds)) : 0
        return value >= 3600 ? String(format: "%d:%02d:%02d", value / 3600, value / 60 % 60, value % 60) : String(format: "%d:%02d", value / 60, value % 60)
    }
}

struct ShareSheet: UIViewControllerRepresentable {
    var items: [Any]
    func makeUIViewController(context: Context) -> UIActivityViewController { UIActivityViewController(activityItems: items, applicationActivities: nil) }
    func updateUIViewController(_ controller: UIActivityViewController, context: Context) {}
}
struct AboutView: View {
    @Environment(\.dismiss) private var dismiss
    var body: some View {
        NavigationStack {
            List {
                Section("Kog \(Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "")") {
                    Text("Music on your server and on your iPhone. Powered by Kog's shared Rust audio and library backend.")
                    Link("Source code", destination: URL(string: "https://github.com/benwbooth/Kog")!)
                }
                Section("Licenses") {
                    ForEach(["LICENSE", "THIRD_PARTY_NOTICES.md"], id: \.self) { file in
                        NavigationLink(file == "LICENSE" ? "Kog license" : "Third-party notices") {
                            ScrollView { Text(resource(file)).font(.caption.monospaced()).textSelection(.enabled).padding() }.navigationTitle("Licenses")
                        }
                    }
                }
            }.navigationTitle("About Kog").toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
        }
    }
    private func resource(_ name: String) -> String {
        guard let url = Bundle.main.url(forResource: name, withExtension: nil), let text = try? String(contentsOf: url, encoding: .utf8) else { return "See the source repository for license texts." }
        return text
    }
}
