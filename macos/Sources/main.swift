// Entry point. No nib, no storyboard: the menus are built in the delegate.

import AppKit

let application = NSApplication.shared
let delegate = AppDelegate()
application.delegate = delegate
// Regular rather than accessory: the window gets a real menu bar, Cmd-Tab and a Dock
// icon, and the status item is added on top of that.
application.setActivationPolicy(.regular)
application.run()
