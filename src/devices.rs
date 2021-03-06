use super::NativeManager;

use std::sync::atomic::{AtomicU32, AtomicUsize, AtomicBool, Ordering};

/// Allow the up to the ridiculous number of 64 physical joysticks.
pub const CONTROLLER_MAX: usize = 64;

#[repr(C)]
struct TimeVal {
    tv_sec: isize,
    tv_usec: isize,
}

#[repr(C)]
struct Event {
    ev_time: TimeVal,
    ev_type: i16,
    ev_code: i16,
    ev_value: i32,
}

/// A button on a controller.
///
/// Example controller:
///
/// <img src="https://libcala.github.io/stick/res/controller.png" width="292">
#[derive(Copy, Clone)]
#[repr(u8)]
pub enum Btn {
    /// D-PAD LEFT / LEFT ARROW KEY / SCROLL UP "Previous Item"
    Left = 0u8,
    /// D-PAD RIGHT / RIGHT ARROW KEY / SCROLL DOWN "Next Item"
    Right = 1,
    /// D-PAD UP / UP ARROW KEY / R KEY "Reload/Tinker"
    Up = 2,
    /// D-PAD DOWN / DOWN ARROW KEY / X KEY "Put Away"
    Down = 3,

    /// ONE OF: Y OR X BUTTON / LEFT CLICK "Action/Attack/Execute/Use Item"
    X = 4,
    /// A BUTTON / ENTER KEY / RIGHT CLICK "Talk/Inspect/Ok/Accept"
    A = 5,
    /// ONE OF: Y OR X BUTTON / SPACE KEY "Jump/Upward"
    Y = 6,
    /// B BUTTON / SHIFT KEY "Speed Things Up/Cancel"
    B = 7,

    /// L THROTTLE BTN / CTRL KEY "Crouch/Sneak"
    L = 8,
    /// R THROTTLE BTN / ALT KEY "Slingshot/Bow & Arrow"
    R = 9,
    /// L BTN / W BTN / BACKSPACE KEY "Throw/Send/Wave"
    W = 10,
    /// R BTN / Z BTN / Z KEY "Alternative Action/Kick"
    Z = 11,

    /// BACK / SELECT / QUIT / ESCAPE KEY / EXIT "Menu / Quit / Finish"
    F = 12,
    /// START / E KEY / MENU / FIND "Inventory/Pockets/Find"
    E = 13,
    /// JOY1 PUSH / C KEY "Toggle Crouch/Sneak"
    D = 14,
    /// JOY2 PUSH / F KEY "Camera/Binoculars"
    C = 15,
}

impl From<Btn> for u8 {
    fn from(b: Btn) -> Self {
        b as u8
    }
}

/// The state of a joystick, gamepad or controller device.
#[derive(Debug)]
pub struct Device {
    native_handle: u32,
    // Hardware ID for this device.
    hardware_id: u32,
    abs_min: i32,
    abs_max: i32,
    // 256 bits total

    // AXIS (Atomic f32)
    joyx: AtomicU32,
    joyy: AtomicU32,
    camx: AtomicU32,
    camy: AtomicU32,
    trgl: AtomicU32,
    trgr: AtomicU32,
    // BTNS (32 bits)
    btns: AtomicU32,
    // Is it plugged in?
    plug: AtomicBool,
}

impl std::fmt::Display for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        let joy: (f32, f32) = (gfloat(&self.joyx), gfloat(&self.joyy));
        let cam: (f32, f32) = (gfloat(&self.camx), gfloat(&self.camy));
        let lrt: (f32, f32) = (gfloat(&self.trgl), gfloat(&self.trgr));

        fn button_symbol(state: Option<bool>) -> char {
            if state == Some(true) {
                '▣'
            } else {
                '□'
            }
        }

        let b_btn: char = button_symbol(self.btn(Btn::B));
        let a_btn: char = button_symbol(self.btn(Btn::A));
        let y_btn: char = button_symbol(self.btn(Btn::Y));
        let x_btn: char = button_symbol(self.btn(Btn::X));

        let dl: char = button_symbol(self.btn(Btn::Left));
        let dr: char = button_symbol(self.btn(Btn::Right));
        let du: char = button_symbol(self.btn(Btn::Up));
        let dd: char = button_symbol(self.btn(Btn::Down));

        let w_btn: char = button_symbol(self.btn(Btn::W));
        let z_btn: char = button_symbol(self.btn(Btn::Z));
        let l_btn: char = button_symbol(self.btn(Btn::L));
        let r_btn: char = button_symbol(self.btn(Btn::R));

        let d_btn: char = button_symbol(self.btn(Btn::D));
        let c_btn: char = button_symbol(self.btn(Btn::C));
        let f_btn: char = button_symbol(self.btn(Btn::F));
        let e_btn: char = button_symbol(self.btn(Btn::E));

        write!(
            f,
            "j({:.2},{:.2}) c({:.2},{:.2}) T({:.2},{:.2}) b{} a{} y{} x{} ←{} →{} \
             ↑{} ↓{} l{} r{} w{} z{} f{} e{} d{} c{}",
            joy.0,
            joy.1,
            cam.0,
            cam.1,
            lrt.0,
            lrt.1,
            b_btn,
            a_btn,
            y_btn,
            x_btn,
            dl,
            dr,
            du,
            dd,
            l_btn,
            r_btn,
            w_btn,
            z_btn,
            f_btn,
            e_btn,
            d_btn,
            c_btn,
        )
    }
}

impl Device {
    /// Get main joystick state from the device if a main joystick exists, otherwise return `None`.
    pub fn joy(&self) -> Option<(f32, f32)> {
        Some((gfloat(&self.joyx), gfloat(&self.joyy)))
    }

    /// Get X & Y from camera stick if it exists, otherwise return `None`.
    pub fn cam(&self) -> Option<(f32, f32)> {
        #[allow(clippy::single_match)]
        match self.hardware_id {
            // Flight controller
            0x_07B5_0316 => return None,
            _ => {}
        }

        Some((gfloat(&self.camx), gfloat(&self.camy)))
    }

    /// Get the left & right trigger values.
    pub fn lrt(&self) -> Option<(f32, f32)> {
        Some((gfloat(&self.trgl), gfloat(&self.trgr)))
    }

    /// Return `Some(true)` if a button is pressed, `Some(false)` if not, and `None` if the button
    /// doesn't exist.
    pub fn btn<B: Into<u8>>(&self, b: B) -> Option<bool> {
        Some(self.btns.load(Ordering::Relaxed) & (1 << (b.into())) != 0)
    }
}

// Adjust atomic float.
fn afloat(float: &AtomicU32, fnc: &dyn Fn(f32) -> f32) {
    let old = f32::from_bits(float.load(Ordering::Relaxed));

    float.store(fnc(old).to_bits(), Ordering::Relaxed);
}

// Get atomic float.
fn gfloat(float: &AtomicU32) -> f32 {
    f32::from_bits(float.load(Ordering::Relaxed))
}

/// An interface to all joystick, gamepad and controller devices.
pub struct Port {
    // Native bindings to file descriptors
    manager: NativeManager,
    // Number of controllers.
    count: AtomicUsize,
    // The controllers' data.
    controllers: [Device; CONTROLLER_MAX],
}

impl Default for Port {
    fn default() -> Self {
        Self::new()
    }
}

impl Port {
    /// Create a new interface to all joystick, gamepad and controller devices currently plugged in
    /// to this computer.
    pub fn new() -> Port {
        let manager = NativeManager::new();
        let controllers = unsafe { std::mem::zeroed() };
        let count = AtomicUsize::new(0);

        let mut port = Port {
            manager,
            count,
            controllers,
        };

        for stick in 0..port.manager.num_plugged_in() {
            port.add_stick(stick);
        }

        port
    }

    fn add_stick(&mut self, index: usize) {
        let (min, max, _) = self.manager.get_abs(index);

        self.count.fetch_add(1, Ordering::Relaxed);

        self.controllers[index] = Device {
            native_handle: index as u32,
            hardware_id: self.manager.get_id(index).0,
            abs_min: min,
            abs_max: max,

            joyx: AtomicU32::new(0),
            joyy: AtomicU32::new(0),
            camx: AtomicU32::new(0),
            camy: AtomicU32::new(0),
            trgl: AtomicU32::new(0),
            trgr: AtomicU32::new(0),
            btns: AtomicU32::new(0),
            plug: AtomicBool::new(true),
        };
    }

    /// Block thread until input is available.
    pub async fn input(&mut self) -> Option<u8> {
        if let Some(fd) = self.manager.async_device {
            if fd == self.manager.async_device.fd() {
                // not a joystick (one's been plugged in).
                let (is_add, index) =
                    crate::ffi::inotify_read(&mut self.manager)?;
                println!("Controller Count Changed {} {}", is_add, index);

                if is_add {
                    // FOR TESTING
                    // println!("s{:08X}", self.manager.get_id(added).0);
                    self.add_stick(index);
                    return Some(index as u8);
                } else {
                    return None;
                }
            }

            for i in 0..self.controllers.len() {
                let (devfd, is_out, ne) = self
                    .manager
                    .get_fd(self.controllers[i].native_handle as usize);

                if ne {
                    continue;
                }

                if is_out {
                    self.count.fetch_sub(1, Ordering::Relaxed);
                    self.manager.disconnect(fd);
                    continue;
                }

                if devfd != fd {
                    continue;
                }

                while joystick_poll_event(fd, &mut self.controllers[i]) {}

                return Some(i as u8);
            }
        }
        None
    }

    /// Get the state of a device
    pub fn get(&self, stick: u8) -> Option<&Device> {
        if self.controllers[stick as usize]
            .plug
            .load(Ordering::Relaxed)
        {
            Some(&self.controllers[stick as usize])
        } else {
            None
        }
    }

    /// Swap two devices in the interface by their indexes.
    /// # Panics
    /// If either `a` or `b` are out of bounds.
    /// # Note
    /// This is useful for if in a game, you want P1 and P2 to swap which controller they are
    /// assigned to.  You can do this with:
    /// ```norun
    /// // Assuming P1 is at index 0, and P2 is at index 1,
    /// devices.swap(0, 1);
    /// ```
    pub fn swap(&mut self, a: u8, b: u8) {
        self.controllers.swap(a as usize, b as usize);
    }

    /// Get the name of a device by index.
    #[allow(unused)]
    pub fn name(&self, a: u8) -> String {
        // TODO
        "Unknown".to_string()
    }

    /// Get the number of plugged in controllers.
    pub fn count(&self) -> u8 {
        self.count.load(Ordering::Relaxed) as u8
    }
}

fn joystick_poll_event(fd: i32, device: &mut Device) -> bool {
    extern "C" {
        fn read(fd: i32, buf: *mut Event, count: usize) -> isize;
    }

    let mut js = std::mem::MaybeUninit::uninit();
    let bytes = unsafe { read(fd, js.as_mut_ptr(), std::mem::size_of::<Event>()) };
    if bytes != (std::mem::size_of::<Event>() as isize) {
        return false;
    }
    let js = unsafe { js.assume_init() };

    fn edit<B: Into<u8>>(is: bool, device: &mut Device, b: B) {
        if is {
            device.btns.fetch_or(1 << b.into(), Ordering::Relaxed);
        } else {
            device.btns.fetch_and(!(1 << b.into()), Ordering::Relaxed);
        }
    }

    // Apply Mods
    let a = if device.hardware_id == 0x_0E6F_0501
    /* XBOX */
    {
        Btn::B
    } else {
        Btn::A
    };

    let b = if device.hardware_id == 0x_0E6F_0501
    /* XBOX */
    {
        Btn::A
    } else {
        Btn::B
    };

    let x = if device.hardware_id == 0x_054C_0268
    /* PS3 */
    {
        Btn::Y
    } else {
        Btn::X
    };

    let y = if device.hardware_id == 0x_054C_0268
    /* PS3 */
    {
        Btn::X
    } else {
        Btn::Y
    };

    // Get Events
    match js.ev_type {
        // button press / release (key)
        0x01 => {
            //            println!("EV CODE {}", js.ev_code - 0x120);

            let is = js.ev_value == 1;

            match js.ev_code - 0x120 {
                // ABXY
                0 | 19 => edit(is, device, x),
                1 | 17 => edit(is, device, a),
                2 | 16 => edit(is, device, b),
                3 | 20 => edit(is, device, y),
                // LT/RT
                4 | 24 => edit(is, device, Btn::L),
                5 | 25 => edit(is, device, Btn::R),
                // LB/RB
                6 | 22 => edit(is, device, Btn::W), // 6 is a guess.
                7 | 23 => edit(is, device, Btn::Z),
                // Select/Start
                8 | 26 => edit(is, device, Btn::F), // 8 is a guess.
                9 | 27 => edit(is, device, Btn::E),
                // ?
                10 => println!("Button 10 is Unknown"),
                // D-PAD
                12 | 256 => edit(is, device, Btn::Up),
                13 | 259 => edit(is, device, Btn::Right),
                14 | 257 => edit(is, device, Btn::Down),
                15 | 258 => edit(is, device, Btn::Left),
                // 16-17 already matched
                18 => println!("Button 18 is Unknown"),
                // 19-20 already matched
                21 => println!("Button 21 is Unknown"),
                // 22-27 already matched
                28 => println!("Button 28 is Unknown"),
                29 => edit(is, device, Btn::D),
                30 => edit(is, device, Btn::C),
                a => println!("Button {} is Unknown", a),
            }
        }
        // axis move (abs)
        0x03 => {
            let value = if device.hardware_id == 0x_0079_1844 {
                // GameCube
                let pad = (device.abs_max - device.abs_min) / 4;
                transform(
                    device.abs_min + pad,
                    device.abs_max - pad,
                    js.ev_value,
                )
            } else {
                transform(device.abs_min, device.abs_max, js.ev_value)
            };

            let value2 = if device.hardware_id == 0x_0079_1844 {
                // GameCube
                transform2(32, 95, js.ev_value)
            } else {
                transform2(0, 127, js.ev_value)
            };

            // if value != 0 {
            //     println!("{} {}", js.ev_code, value);
            // }

            // For some reason this is different on the GameCube controller, so fix it.
            let (cam_x, cam_y, lrt_l, lrt_r) = match device.hardware_id {
                0x_0079_1844 => (5, 2, 3, 4),
                _ => (3, 4, 2, 5),
            };

            match js.ev_code {
                0 => afloat(&device.joyx, &|_| value),
                1 => afloat(&device.joyy, &|_| value),
                16 => {
                    if js.ev_value < 0 {
                        edit(true, device, Btn::Left);
                        edit(false, device, Btn::Right);
                    } else if js.ev_value > 0 {
                        edit(false, device, Btn::Left);
                        edit(true, device, Btn::Right);
                    } else {
                        edit(false, device, Btn::Left);
                        edit(false, device, Btn::Right);
                    }
                }
                17 => {
                    if js.ev_value < 0 {
                        edit(true, device, Btn::Up);
                        edit(false, device, Btn::Down);
                    } else if js.ev_value > 0 {
                        edit(false, device, Btn::Up);
                        edit(true, device, Btn::Down);
                    } else {
                        edit(false, device, Btn::Up);
                        edit(false, device, Btn::Down);
                    }
                }
                40 => {} // IGNORE: Duplicate axis.
                a => {
                    if a == cam_x {
                        afloat(&device.camx, &|_| {
                            value
                        });
                    } else if a == cam_y {
                        afloat(&device.camy, &|_| {
                            value
                        });
                    } else if a == lrt_l {
                        if value2 > 0.99 {
                            edit(true, device, Btn::L)
                        } else {
                            edit(false, device, Btn::L)
                        }
                        afloat(&device.trgl, &|_| {
                            value2
                        });
                    } else if a == lrt_r {
                        if value2 > 0.99 {
                            edit(true, device, Btn::R)
                        } else {
                            edit(false, device, Btn::R)
                        }
                        afloat(&device.trgr, &|_| {
                            value2
                        });
                    }
                } // println!("Unknown Axis: {}", a),
            }
        }
        // ignore
        _ => {}
    }

    true
}

fn deadzone(min: i32, max: i32, val: i32) -> (i32, i32) {
    let range = max - min;
    let halfr = range >> 1;
    let deadz = halfr >> 2; // 1/8th = deadzone.
    let midpt = min + halfr;
    // Center the range.
    let value = val - midpt; // -halfr to halfr
                             // Take deadzone into account.
    let value = if value < deadz {
        if value > -deadz {
            0
        } else {
            value + deadz
        }
    } else {
        value - deadz
    };
    (value, (range >> 1) - deadz)
}

fn transform(min: i32, max: i32, val: i32) -> f32 {
    let (value, full) = deadzone(min, max, val);
    // Modify integer range from (-(full) thru (full)) to -127 to 127
    ((value * 127) / full).max(-127).min(127) as f32 / 127.0
}

fn transform2(min: i32, max: i32, val: i32) -> f32 {
    // Modify integer range from (-(full) thru (full)) to 0 to 255
    ((val * 255) / (max - min)).max(0).min(255) as f32 / 255.0
}

#[cfg(test)]
mod tests {
    /*    use super::*;

    #[test]
    fn transform_test() {
        let a = deadzone(-100, 100, 100);
        assert_eq!(a.0, a.1);
        assert_eq!(75, a.1);
        let b = deadzone(-100, 100, -100);
        assert_eq!(b.0, -b.1);
        assert_eq!(75, b.1);
        let c = deadzone(-100, 100, 0);
        assert_eq!(c.0, 0);
        assert_eq!(75, b.1);

        assert_eq!(transform(-100, 100, 100), 127);
        assert_eq!(transform(-100, 100, -100), -127);
        assert_eq!(transform(-100, 100, 0), 0);

        assert_eq!(transform(-128, 127, 127), 127);
        assert_eq!(transform(-128, 127, 0), 0);
        assert_eq!(transform(-128, 127, -128), -127);
    }*/
}
