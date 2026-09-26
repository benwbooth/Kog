import SwiftUI

@main
struct KogApp: App {
    @StateObject private var store = KogStore()

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(store)
                .preferredColorScheme(.dark)
                #if KOG_DEVICE_TESTS && KOG_NATIVE_AUDIO
                .task { await DeviceVerification.runIfRequested(store) }
                #endif
        }
    }
}
