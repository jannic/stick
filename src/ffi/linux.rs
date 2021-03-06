use smelling_salts::{Watcher, Device as AsyncDevice};

use std::fs;
use std::mem;

// use crate::devices::MAX_JS;

extern "C" {
    fn open(pathname: *const u8, flags: i32) -> i32;
    fn close(fd: i32) -> i32;
    fn fcntl(fd: i32, cmd: i32, v: i32) -> i32;
}

#[repr(C)]
struct Device {
    name: [u8; 256 + 17],
    async_device: AsyncDevice,
}

pub struct NativeManager {
    // Inotify Device.
    pub(crate) async_device: AsyncDevice,
    // Controller File Descriptors.
    devices: Vec<Device>,
}

impl NativeManager {
    pub fn new() -> NativeManager {
        let inotify = inotify_new();
        let watcher = Watcher::new().input();
        let async_device = AsyncDevice::new(inotify, watcher);

        let mut nm = NativeManager {
            async_device,
            devices: Vec::new(),
        };

        // Look for joysticks immediately.
        let paths = fs::read_dir("/dev/input/by-id/");
        let paths = if let Ok(paths) = paths {
            paths
        } else {
            return nm;
        };

        for path in paths {
            let path_str = path.unwrap().path();
            let path_str = path_str.file_name().unwrap();
            let path_str = path_str.to_str().unwrap();

            // An evdev device.
            if path_str.ends_with("-event-joystick") {
                let mut event = Event {
                    wd: 0,       /* Watch descriptor */
                    mask: 0x100, /* Mask describing event */
                    cookie: 0,   /* Unique cookie associating related
                                 events (for rename(2)) */
                    len: 0,         /* Size of name field */
                    name: [0; 256], /* Optional null-terminated name */
                };

                let path_str = path_str.to_string().into_bytes();
                let slice_len = path_str.len().min(255);

                event.name[..slice_len]
                    .clone_from_slice(&path_str[..slice_len]);

                inotify_read2(&mut nm, event);
            }
        }

        nm
    }

    pub fn get_id(&self, id: usize) -> (u32, bool) {
        if id >= self.devices.len() {
            (0, true)
        } else {
            let (a, b) = joystick_id(self.devices[id].async_device.fd());

            (a, b)
        }
    }

    pub fn get_abs(&self, id: usize) -> (i32, i32, bool) {
        if id >= self.devices.len() {
            (0, 0, true)
        } else {
            joystick_abs(self.devices[id].async_device.fd())
        }
    }

    pub fn get_fd(&self, id: usize) -> (i32, bool, bool) {
        let (_, unplug) = self.get_id(id);

        (
            self.devices[id].async_device.fd(),
            unplug,
            self.devices[id].name[0] == b'\0',
        )
    }

    pub fn num_plugged_in(&self) -> usize {
        self.devices.len()
    }

    pub fn disconnect(&mut self, fd: i32) -> usize {
        for i in 0..self.devices.len() {
            if self.devices[i].async_device.fd() == fd {
                self.async_device.old();
                joystick_drop(fd);
                self.devices[i].name[0] = b'\0';
                return i;
            }
        }

        panic!("There was no fd of {}", fd);
    }
}
impl Drop for NativeManager {
    fn drop(&mut self) {
        while let Some(device) = self.devices.pop() {
            self.disconnect(device.async_device.fd());
        }
        unsafe {
            let fd = self.async_device.fd();
            self.async_device.old();
            close(fd);
        }
    }
}

// Set up file descriptor for asynchronous reading.
fn joystick_async(fd: i32) {
    let error = unsafe { fcntl(fd, 0x4, 0x800) } == -1;

    if error {
        panic!("Joystick unplugged 2!");
    }
}

// Get the joystick id.
fn joystick_id(fd: i32) -> (u32, bool) {
    let mut a = [0u16; 4];

    extern "C" {
        fn ioctl(fd: i32, request: usize, v: *mut u16) -> i32;
    }

    if unsafe { ioctl(fd, 0x_8008_4502, &mut a[0]) } == -1 {
        return (0, true);
    }

    (((u32::from(a[1])) << 16) | (u32::from(a[2])), false)
}

fn joystick_abs(fd: i32) -> (i32, i32, bool) {
    #[derive(Debug)]
    #[repr(C)]
    struct AbsInfo {
        value: i32,
        minimum: i32,
        maximum: i32,
        fuzz: i32,
        flat: i32,
        resolution: i32,
    }

    extern "C" {
        fn ioctl(fd: i32, request: usize, v: *mut AbsInfo) -> i32;
    }

    let mut a = mem::MaybeUninit::uninit();
    let a = unsafe {
        if ioctl(fd, 0x_8018_4540, a.as_mut_ptr()) == -1 {
            return (0, 0, true);
        }
        a.assume_init()
    };

    (a.minimum, a.maximum, false)
}

// Disconnect the joystick.
fn joystick_drop(fd: i32) {
    if unsafe { close(fd) == -1 } {
        panic!("Failed to disconnect joystick.");
    }
}

fn inotify_new() -> i32 {
    extern "C" {
        fn inotify_init() -> i32;
        fn inotify_add_watch(fd: i32, pathname: *const u8, mask: u32) -> i32;
    }

    let fd = unsafe { inotify_init() };

    if fd == -1 {
        panic!("Couldn't create inotify (1)!");
    }

    if unsafe {
        inotify_add_watch(
            fd,
            b"/dev/input/by-id/\0".as_ptr() as *const _,
            0x0000_0100 | 0x0000_0200,
        )
    } == -1
    {
        panic!("Couldn't create inotify (2)!");
    }

    fd
}

#[repr(C)]
struct Event {
    wd: i32,   /* Watch descriptor */
    mask: u32, /* Mask describing event */
    cookie: u32, /* Unique cookie associating related
               events (for rename(2)) */
    len: u32,        /* Size of name field */
    name: [u8; 256], /* Optional null-terminated name */
}

// Add or remove joystick
fn inotify_read2(port: &mut NativeManager, ev: Event) -> Option<(bool, usize)> {
    let mut name = [0; 256 + 17];
    name[0] = b'/';
    name[1] = b'd';
    name[2] = b'e';
    name[3] = b'v';
    name[4] = b'/';
    name[5] = b'i';
    name[6] = b'n';
    name[7] = b'p';
    name[8] = b'u';
    name[9] = b't';
    name[10] = b'/';
    name[11] = b'b';
    name[12] = b'y';
    name[13] = b'-';
    name[14] = b'i';
    name[15] = b'd';
    name[16] = b'/';
    let mut length = 0;
    for i in 0..256 {
        name[i + 17] = ev.name[i];
        if ev.name[i] == b'\0' {
            length = i + 17;
            break;
        }
    }

    let namer = String::from_utf8_lossy(&name[0..length]);
    let mut fd = unsafe { open(name.as_ptr() as *const _, 0) };
    if !namer.ends_with("-event-joystick") || ev.mask != 0x0000_0100 {
        return None;
    }

    if fd == -1 {
        // Avoid race condition
        std::thread::sleep(std::time::Duration::from_millis(16));
        fd = unsafe { open(name.as_ptr() as *const _, 0) };
        if fd == -1 {
            return None;
        }
    }

    joystick_async(fd);
    let async_device = AsyncDevice::new(fd, Watcher::new().input());
    let device = Device { name, async_device };

    for i in 0..port.devices.len() {
        if port.devices[i].name[0] == b'\0' {
            port.devices[i] = device;
            return Some((true, i));
        }
    }

    port.devices.push(device);
    Some((true, port.devices.len() - 1))
}

// Read joystick add or remove event.
pub(crate) fn inotify_read(port: &mut NativeManager) -> Option<(bool, usize)> {
    extern "C" {
        fn read(fd: i32, buf: *mut Event, count: usize) -> isize;
    }

    let mut ev = mem::MaybeUninit::uninit();
    let ev = unsafe {
        read(port.async_device.fd(), ev.as_mut_ptr(), mem::size_of::<Event>());
        ev.assume_init()
    };

    inotify_read2(port, ev)
}
