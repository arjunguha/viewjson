import SwiftUI

@main
struct slopjsonApp: App {
    @StateObject private var viewModel = ViewerViewModel()

    var body: some Scene {
        WindowGroup {
            ContentView(viewModel: viewModel)
        }
        .commands {
            CommandGroup(after: .importExport) {
                Button("Open Files…") {
                    viewModel.presentOpenPanel()
                }
                .keyboardShortcut("o", modifiers: .command)

                Button("Paste JSON") {
                    viewModel.pasteFromClipboard()
                }
                .keyboardShortcut("v", modifiers: [.command, .shift])

                Divider()

                Button("Clear Workspace") {
                    viewModel.clearAll()
                }
                .keyboardShortcut(.delete, modifiers: .command)
                .disabled(viewModel.selectedNode == nil && viewModel.documents.isEmpty)
            }
        }
    }
}
