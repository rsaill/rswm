use x11::xlib;
use x11::keysym;
use std::os::raw;
use std::collections::hash_map;

#[derive(Default)]
pub struct Position { x:i32, y:i32 }

static X_REQUEST_CODE_NAMES : [&str;121] = [
      "",
      "CreateWindow",
      "ChangeWindowAttributes",
      "GetWindowAttributes",
      "DestroyWindow",
      "DestroySubwindows",
      "ChangeSaveSet",
      "ReparentWindow",
      "MapWindow",
      "MapSubwindows",
      "UnmapWindow",
      "UnmapSubwindows",
      "ConfigureWindow",
      "CirculateWindow",
      "GetGeometry",
      "QueryTree",
      "InternAtom",
      "GetAtomName",
      "ChangeProperty",
      "DeleteProperty",
      "GetProperty",
      "ListProperties",
      "SetSelectionOwner",
      "GetSelectionOwner",
      "ConvertSelection",
      "SendEvent",
      "GrabPointer",
      "UngrabPointer",
      "GrabButton",
      "UngrabButton",
      "ChangeActivePointerGrab",
      "GrabKeyboard",
      "UngrabKeyboard",
      "GrabKey",
      "UngrabKey",
      "AllowEvents",
      "GrabServer",
      "UngrabServer",
      "QueryPointer",
      "GetMotionEvents",
      "TranslateCoords",
      "WarpPointer",
      "SetInputFocus",
      "GetInputFocus",
      "QueryKeymap",
      "OpenFont",
      "CloseFont",
      "QueryFont",
      "QueryTextExtents",
      "ListFonts",
      "ListFontsWithInfo",
      "SetFontPath",
      "GetFontPath",
      "CreatePixmap",
      "FreePixmap",
      "CreateGC",
      "ChangeGC",
      "CopyGC",
      "SetDashes",
      "SetClipRectangles",
      "FreeGC",
      "ClearArea",
      "CopyArea",
      "CopyPlane",
      "PolyPoint",
      "PolyLine",
      "PolySegment",
      "PolyRectangle",
      "PolyArc",
      "FillPoly",
      "PolyFillRectangle",
      "PolyFillArc",
      "PutImage",
      "GetImage",
      "PolyText8",
      "PolyText16",
      "ImageText8",
      "ImageText16",
      "CreateColormap",
      "FreeColormap",
      "CopyColormapAndFree",
      "InstallColormap",
      "UninstallColormap",
      "ListInstalledColormaps",
      "AllocColor",
      "AllocNamedColor",
      "AllocColorCells",
      "AllocColorPlanes",
      "FreeColors",
      "StoreColors",
      "StoreNamedColor",
      "QueryColors",
      "LookupColor",
      "CreateCursor",
      "CreateGlyphCursor",
      "FreeCursor",
      "RecolorCursor",
      "QueryBestSize",
      "QueryExtension",
      "ListExtensions",
      "ChangeKeyboardMapping",
      "GetKeyboardMapping",
      "ChangeKeyboardControl",
      "GetKeyboardControl",
      "Bell",
      "ChangePointerControl",
      "GetPointerControl",
      "SetScreenSaver",
      "GetScreenSaver",
      "ChangeHosts",
      "ListHosts",
      "SetAccessControl",
      "SetCloseDownMode",
      "KillClient",
      "RotateProperties",
      "ForceScreenSaver",
      "SetPointerMapping",
      "GetPointerMapping",
      "SetModifierMapping",
      "GetModifierMapping",
      "NoOperation",
      ];

pub struct WindowManager {
    display: *mut xlib::Display,
    root: raw::c_ulong,
    clients: hash_map::HashMap<xlib::Window, xlib::Window>,
    clients_vec: Vec<xlib::Window>,
    drag_start_pos: Position,
    drag_start_frame_pos: Position,
    drag_start_frame_size: Position,
    wm_protocols: xlib::Atom,
    wm_delete_window: xlib::Atom,
}

fn in_list(ptr:*mut xlib::Atom, size: i32, elt: xlib::Atom) -> bool {
    for i in 0..size {
        if unsafe { *ptr.offset(i as isize) } == elt { return true }
    }
    return false
}

impl WindowManager {

    fn create() -> WindowManager {
        let nullptr : *const std::os::raw::c_char = std::ptr::null();
        unsafe {
            let display = xlib::XOpenDisplay(nullptr);
            let root = xlib::XDefaultRootWindow(display);
            let wm_protocols_cstring : std::ffi::CString = std::ffi::CString::new("WM_PROTOCOLS").expect("CString::new() failed");
            let wm_delete_window_cstring : std::ffi::CString = std::ffi::CString::new("WM_DELETE_WINDOW").expect("CString::new() failed");
            return WindowManager { 
                display: display,
                root: root,
                clients: hash_map::HashMap::new(),
                clients_vec: Vec::new(),
                drag_start_pos: Default::default(),
                drag_start_frame_pos: Default::default(),
                drag_start_frame_size: Default::default(),
                wm_protocols: xlib::XInternAtom(display, wm_protocols_cstring.as_ptr(), 0),
                wm_delete_window: xlib::XInternAtom(display, wm_delete_window_cstring.as_ptr(), 0),
            }
        }
    }

    unsafe extern "C" fn on_wm_detected(_: *mut xlib::Display, e: *mut xlib::XErrorEvent) -> raw::c_int {
        assert_eq!((*e).error_code,xlib::BadAccess);
        panic!("Detected another window manager");
    }


    unsafe extern "C" fn on_xerror(display: *mut xlib::Display, e: *mut xlib::XErrorEvent) -> raw::c_int {
        let mut buffer : [i8;1024] = [1;1024];
        let buffer_ptr = buffer.as_mut_ptr();
        xlib::XGetErrorText(
            display,
            (*e).error_code as i32,
            buffer_ptr,
            std::mem::size_of::<[i8;1024]>() as i32);
        eprintln!("Received X error:\nRequest: {} - {}\nError code: {} - {}\nResource ID: {}",
                  (*e).request_code,
                  X_REQUEST_CODE_NAMES[(*e).request_code as usize],
                  (*e).error_code,
                  std::ffi::CStr::from_ptr(buffer_ptr).to_str().unwrap(),
                  (*e).resourceid);
        return 1
    }


    fn frame(&mut self, w: xlib::Window, was_created_before_window_manager: bool) {
        // Visual properties of the frame to create.
        let border_width : u32 = 3;
        let border_color : u64 = 0xff0000;
        let bg_color : u64 = 0x0000ff;

        // We shouldn't be framing windows we've already framed.
        assert!(!self.clients.contains_key(&w));

        // 1. Retrieve attributes of window to frame.
        let mut x_window_attrs = xlib::XWindowAttributes {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            border_width: 0,
            depth: 0,
            visual: std::ptr::null_mut(),
            root: 0,
            class: 0,
            bit_gravity: 0,
            win_gravity: 0,
            backing_store: 0,
            backing_planes: 0,
            backing_pixel: 0,
            save_under: 0,
            colormap: 0,
            map_installed: 0,
            map_state: 0,
            all_event_masks: 0,
            your_event_mask: 0,
            do_not_propagate_mask: 0,
            override_redirect: 0,
            screen: std::ptr::null_mut(),
        };

        unsafe {
            let i = xlib::XGetWindowAttributes(self.display, w, &mut x_window_attrs);
            assert!(i > 0)
        };

        // 2. If window was created before window manager started, we should frame
        // it only if it is visible and doesn't set override_redirect.
        if was_created_before_window_manager {
            if x_window_attrs.override_redirect > 0 || x_window_attrs.map_state != xlib::IsViewable {
                return;
            }
        }

        // 3. Create frame.
        unsafe {
            let frame = xlib::XCreateSimpleWindow(
                self.display,
                self.root,
                x_window_attrs.x,
                x_window_attrs.y,
                x_window_attrs.width as u32,
                x_window_attrs.height as u32,
                border_width,
                border_color,
                bg_color);
            // 4. Select events on frame.
            xlib::XSelectInput( self.display, frame, xlib::SubstructureRedirectMask | xlib::SubstructureNotifyMask);
            // 5. Add client to save set, so that it will be restored and kept alive if we
            // crash.
            xlib::XAddToSaveSet(self.display, w);
            // 6. Reparent client window.
            xlib::XReparentWindow( self.display, w, frame, 0, 0);  // Offset of client window within frame.
            // 7. Map frame.
            xlib::XMapWindow(self.display, frame);
            // 8. Save frame handle.
            self.clients.insert(w,frame);
            self.clients_vec.push(w);
            // 9. Grab universal window management actions on client window.
            //   a. Move windows with ctrl + left button.
            xlib::XGrabButton(
                self.display,
                xlib::Button1,
                xlib::ControlMask,
                w,
                0,
                (xlib::ButtonPressMask | xlib::ButtonReleaseMask | xlib::ButtonMotionMask) as u32,
                xlib::GrabModeAsync,
                xlib::GrabModeAsync,
                0,
                0);
            //   b. Resize windows with ctrl + right button.
            xlib::XGrabButton(
                self.display,
                xlib::Button3,
                xlib::ControlMask,
                w,
                0,
                (xlib::ButtonPressMask | xlib::ButtonReleaseMask | xlib::ButtonMotionMask) as u32,
                xlib::GrabModeAsync,
                xlib::GrabModeAsync,
                0,
                0);
            //   c. Kill windows with ctrl + f4.
            xlib::XGrabKey(
                self.display,
                xlib::XKeysymToKeycode(self.display, x11::keysym::XK_F4 as u64) as i32,
                xlib::ControlMask,
                w,
                0,
                xlib::GrabModeAsync,
                xlib::GrabModeAsync);
            //   d. Switch windows with ctrl + tab.
            xlib::XGrabKey(
                self.display,
                xlib::XKeysymToKeycode(self.display, x11::keysym::XK_Tab as u64) as i32,
                xlib::ControlMask,
                w,
                0,
                xlib::GrabModeAsync,
                xlib::GrabModeAsync);

            eprintln!("Framed window {} [{}]",w,frame);
        }

    }

    fn unframe(&mut self, w: xlib::Window) {
        // We reverse the steps taken in Frame().
        match self.clients.get(&w){
            None => panic!("unframe"),
            Some(frame) =>
            {
                unsafe {
                    // 1. Unmap frame.
                    xlib::XUnmapWindow(self.display, *frame);
                    // 2. Reparent client window.
                    //println!("reparent (1)");
                    //xlib::XReparentWindow( self.display, w, self.root, 0, 0);  // Offset of client window within root.
                    //x11::xlib::XSync(self.display, 0);
                    //println!("reparent (2)");
                    // 3. Remove client window from save set, as it is now unrelated to us.
                    //xlib::XRemoveFromSaveSet(self.display, w);
                    // 4. Destroy frame.
                    xlib::XDestroyWindow(self.display, *frame);
                }
                // 5. Drop reference to frame handle.
                eprintln!("Unframed window {} [{}]",w,frame);
                self.clients.remove(&w);
                self.clients_vec.retain(|&x| x != w);
            }
        };
    }


    fn on_create_notify(&self, _: &xlib::XCreateWindowEvent) {}

    fn on_destroy_notify(&self, _: &xlib::XDestroyWindowEvent) {}

    fn on_reparent_notify(&self, _: &xlib::XReparentEvent) {}

    fn on_map_notify(&self, _: &xlib::XMapEvent) {}

    fn on_unnmap_notify(&mut self, e: &xlib::XUnmapEvent) -> () {
        // If the window is a client window we manage, unframe it upon UnmapNotify. We
        // need the check because we will receive an UnmapNotify event for a frame
        // window we just destroyed ourselves.
        if !self.clients.contains_key(&e.window) {
            eprintln!("Ignore UnmapNotify for non-client window {}",e.window);
            return;
        }

        // Ignore event if it is triggered by reparenting a window that was mapped
        // before the window manager started.
        //
        // Since we receive UnmapNotify events from the SubstructureNotify mask, the
        // event attribute specifies the parent window of the window that was
        // unmapped. This means that an UnmapNotify event from a normal client window
        // should have this attribute set to a frame window we maintain. Only an
        // UnmapNotify event triggered by reparenting a pre-existing window will have
        // this attribute set to the root window.
        if e.event == self.root {
            eprintln!("Ignore UnmapNotify for reparented pre-existing window {}",e.window);
            return;
        }

        self.unframe(e.window);
    }

    fn on_configure_notify(&self, _: &xlib::XConfigureEvent) -> () { }

    fn on_map_request(&mut self, e: &xlib::XMapRequestEvent) -> () {
        // 1. Frame or re-frame window.
        self.frame(e.window, false);
        // 2. Actually map window.
        unsafe { xlib::XMapWindow(self.display, e.window) };
        return
    }

    fn on_configure_request(&self, e: &xlib::XConfigureRequestEvent) -> () {
        let mut changes = xlib::XWindowChanges {
            x : e.x,
            y : e.y,
            width : e.width,
            height : e.height,
            border_width : e.border_width,
            sibling : e.above,
            stack_mode : e.detail
        };
        match self.clients.get(&e.window) {
            None => {},
            Some (frame) =>
            {
                unsafe { xlib::XConfigureWindow(self.display, *frame, e.value_mask as u32, &mut changes) };
                eprintln!("Resize [{}] to ({},{})",frame,e.width,e.height);
            }
        }
        unsafe { xlib::XConfigureWindow(self.display, e.window, e.value_mask as u32, &mut changes) };
        eprintln!("Resize [{}] to ({},{})",e.window,e.width,e.height);
    }

    fn on_button_press(&mut self, e: &xlib::XButtonEvent) -> () {
        match self.clients.get(&e.window){
            None => panic!("on_button_press"),
            Some(frame) =>
            {
                // 1. Save initial cursor position.
                self.drag_start_pos = Position { x:e.x_root, y:e.y_root };

                // 2. Save initial window info.
                let mut returned_root : xlib::Window = 0;
                let mut x = 0;
                let mut y = 0;
                let mut width = 0;
                let mut height = 0;
                let mut border_width = 0;
                let mut depth = 0;
                unsafe {
                    assert!(xlib::XGetGeometry(
                            self.display,
                            *frame,
                            &mut returned_root,
                            &mut x, &mut y,
                            &mut width, &mut height,
                            &mut border_width,
                            &mut depth) > 0);
                }
                self.drag_start_frame_pos = Position{x, y};
                self.drag_start_frame_size = Position{x:width as i32, y:height as i32};

                // 3. Raise clicked window to top.
                unsafe { xlib::XRaiseWindow(self.display, *frame); }

            }
        }
    }

    fn on_button_release(&self, _: &xlib::XButtonEvent) -> () { }

    
    fn on_key_press(&self, e: &xlib::XKeyEvent) -> () {

        if (e.state & xlib::ControlMask > 0) &&
            (e.keycode == unsafe { xlib::XKeysymToKeycode(self.display, keysym::XK_F4 as u64) as u32 }) {
                // ctrl + f4: Close window.
                //
                // There are two ways to tell an X window to close. The first is to send it
                // a message of type WM_PROTOCOLS and value WM_DELETE_WINDOW. If the client
                // has not explicitly marked itself as supporting this more civilized
                // behavior (using XSetWMProtocols()), we kill it with XKillClient().
                let mut supported_protocols: *mut xlib::Atom = std::ptr::null_mut();
                let mut num_supported_protocols = 0;
                if unsafe { xlib::XGetWMProtocols(self.display, e.window, &mut supported_protocols, &mut num_supported_protocols) } > 0
                    && in_list(supported_protocols, num_supported_protocols, self.wm_delete_window)
                    {
                        eprintln!("Gracefully deleting window {}",e.window);
                        // 1. Construct message.
                        let mut data = xlib::ClientMessageData::new();
                        data.set_long(0,self.wm_delete_window as i64);
                        let mut msg = xlib::XEvent {
                            client_message: xlib::XClientMessageEvent {
                                type_: xlib::ClientMessage,
                                message_type: self.wm_protocols,
                                window: e.window,
                                format: 32,
                                data: data,
                                send_event: 0,
                                display: 0 as *mut xlib::Display,
                                serial: 0,
                            }
                        };
                        // 2. Send message to window to be closed.
                        unsafe { assert!(xlib::XSendEvent(self.display, e.window, 0, 0, &mut msg) > 0) };
                    } else {
                        eprintln!("Killing window {}",e.window);
                        unsafe { xlib::XKillClient(self.display, e.window) };
                    }
            } else if (e.state & xlib::ControlMask) > 0 &&
                (e.keycode == unsafe { xlib::XKeysymToKeycode(self.display, keysym::XK_Tab as u64) as u32 }) {
                    // ctrl + tab: Switch window.
                    // 1. Find next window.
                    let next =
                        match self.clients_vec.iter().position(|&x| x == e.window) {
                            None => { panic!("") }
                            Some(i) =>
                             if i+1 < self.clients_vec.len() { self.clients_vec[i+1] }
                             else { self.clients_vec[0] }
                            
                        };
                    let frame = self.clients.get(&next).unwrap();
                    unsafe {
                        xlib::XRaiseWindow(self.display, *frame);
                        xlib::XSetInputFocus(self.display, next, xlib::RevertToPointerRoot, xlib::CurrentTime);
                    }
                }
    }

    fn on_key_release(&self, _: &xlib::XKeyEvent) -> () { }

    fn on_motion_notify(&self, e: &xlib::XMotionEvent) -> () {
        match self.clients.get(&e.window){
            None => panic!("on_motion_notify"),
            Some(frame) =>
            {
                let drag_pos = Position { x:e.x_root, y:e.y_root };
                let delta_x = drag_pos.x - self.drag_start_pos.x;
                let delta_y = drag_pos.y - self.drag_start_pos.y;

                if (e.state & xlib::Button1Mask) > 0 {
                    // ctrl + left button: Move window.
                    let dest_frame_pos_x = self.drag_start_frame_pos.x + delta_x;
                    let dest_frame_pos_y = self.drag_start_frame_pos.y + delta_y;
                    unsafe {xlib::XMoveWindow( self.display, *frame, dest_frame_pos_x, dest_frame_pos_y) };
                } else if (e.state & xlib::Button3Mask) > 0 {
                    // ctrl + right button: Resize window.
                    // Window dimensions cannot be negative.
                    let new_width:i32 = self.drag_start_frame_size.x + delta_x;
                    let new_width2:u32 = if new_width > 0 { new_width as u32 } else { 0 };
                    let new_height = self.drag_start_frame_size.y + delta_y;
                    let new_height2:u32 = if new_height > 0 { new_height as u32 } else { 0 };
                    unsafe {
                        // 1. Resize frame.
                        xlib::XResizeWindow( self.display, *frame, new_width2, new_height2);
                        // 2. Resize client window.
                        xlib::XResizeWindow( self.display, e.window, new_width2, new_height2);
                    }
                }
            }
        }
    }

    fn run(&mut self) -> () {
        unsafe { 
            x11::xlib::XSetErrorHandler(Some(WindowManager::on_wm_detected));
            x11::xlib::XSelectInput( self.display, self.root, x11::xlib::SubstructureRedirectMask | x11::xlib::SubstructureNotifyMask);
            x11::xlib::XSync(self.display, 0);
            //
            x11::xlib::XSetErrorHandler(Some(WindowManager::on_xerror));
            //   c. Grab X server to prevent windows from changing under us.
            x11::xlib::XGrabServer(self.display);
            //   d. Reparent existing top-level windows.
            //     i. Query existing top-level windows.
            let mut returned_root: x11::xlib::Window = 0;
            let mut returned_parent: x11::xlib::Window = 0;
            let mut top_level_windows : *mut x11::xlib::Window = std::ptr::null_mut();
            let mut num_top_level_windows: u32 = 0;
            assert!(x11::xlib::XQueryTree(
                    self.display,
                    self.root,
                    &mut returned_root,
                    &mut returned_parent,
                    &mut top_level_windows,
                    &mut num_top_level_windows) > 0);

            assert_eq!(returned_root, self.root);
            //     ii. Frame each top-level window.
            for i in 1..num_top_level_windows {
                self.frame(*top_level_windows.add(i as usize), true);
            }
            //     iii. Free top-level window array.
            x11::xlib::XFree(top_level_windows as *mut std::ffi::c_void);
            //   e. Ungrab X server.
            x11::xlib::XUngrabServer(self.display);
        }
        // 2. Main event loop.
        eprintln!("Entering main loop.");
        loop {
            // 1. Get next event.
            let mut e: xlib::XEvent = xlib::XEvent { pad:[0;24] };
            eprintln!("Waiting for next event");
            unsafe { xlib::XNextEvent(self.display, &mut e) };
            eprintln!("Received event: {:?}", e);

            // 2. Dispatch event.
            match e.get_type() {
                xlib::CreateNotify => 
                {
                    self.on_create_notify(e.as_ref())
                },
                xlib::DestroyNotify => 
                {
                    self.on_destroy_notify(e.as_ref())
                },
                xlib::ReparentNotify => 
                {
                    self.on_reparent_notify(e.as_ref())
                },
                xlib::MapNotify =>
                {
                    self.on_map_notify(e.as_ref())
                },
                xlib::UnmapNotify =>
                {
                    self.on_unnmap_notify(e.as_ref())
                },
                xlib::ConfigureNotify =>
                {
                    self.on_configure_notify(e.as_ref());
                },
                xlib::MapRequest =>
                {
                    self.on_map_request(e.as_ref());
                },
                xlib::ConfigureRequest =>
                {
                    self.on_configure_request(e.as_ref());
                },
                xlib::ButtonPress =>
                {
                    self.on_button_press(e.as_ref());
                },
                xlib::ButtonRelease =>
                {
                    self.on_button_release(e.as_ref());
                },
                xlib::MotionNotify =>
                {
                    // Skip any already pending motion events.
                    let window = {
                        let motion : &xlib::XMotionEvent = e.as_ref();
                        motion.window
                    };
                    while unsafe { xlib::XCheckTypedWindowEvent(self.display, window, xlib::MotionNotify, &mut e) } > 0
                    {}
                    self.on_motion_notify(e.as_ref());
                },
                xlib::KeyPress =>
                {
                    self.on_key_press(e.as_ref());
                },
                xlib::KeyRelease =>
                {
                    self.on_key_release(e.as_ref());
                },
                _ =>
                    eprintln!("Ignored event")
            }
        }
    }
}

fn main() {
    let mut wm = WindowManager::create();
    wm.run();
}
