import SwiftUI

@main
struct CollabClientApp: App {
    @StateObject private var vm = AppViewModel()
    @Environment(\.scenePhase) private var scenePhase

    var body: some Scene {
        WindowGroup {
            if vm.config.setupComplete {
                DashboardView(vm: vm)
                    .onAppear { vm.startDashboard() }
                    .onDisappear { vm.stopDashboard() }
            } else {
                SetupView(vm: vm)
            }
        }
        .onChange(of: scenePhase) { _, phase in
            guard vm.config.setupComplete else { return }
            switch phase {
            case .active:
                vm.startDashboard()
            case .background:
                vm.stopDashboard()
            default:
                break
            }
        }
    }
}
