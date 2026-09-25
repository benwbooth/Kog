import SwiftUI

@main
struct KogApp: App {
    @StateObject private var store = KogStore()

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(store)
                .preferredColorScheme(.dark)
        }
    }
}
