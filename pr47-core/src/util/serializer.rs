use std::collections::HashMap;
use std::future::Future;
use std::mem::{ManuallyDrop, transmute};
use std::sync::Arc;

use unchecked_unwrap::UncheckedUnwrap;

use crate::util::async_utils::{Mutex, MutexGuard, oneshot, task, yield_now};

pub struct SharedContext {
    next_task_id: u32,
    running_tasks: ManuallyDrop<HashMap<u32, oneshot::Receiver<()>>>
}

impl SharedContext {
    pub fn new() -> Self {
        Self {
            next_task_id: 0,
            running_tasks: ManuallyDrop::new(HashMap::new())
        }
    }

    pub fn alloc_task_id(&mut self) -> u32 {
        let r: u32 = self.next_task_id;
        self.next_task_id += 1;
        r
    }

    pub fn get_all_tasks(&mut self) -> HashMap<u32, oneshot::Receiver<()>> {
        let r: HashMap<u32, oneshot::Receiver<()>> = unsafe {
            ManuallyDrop::take(&mut self.running_tasks)
        };
        self.running_tasks = ManuallyDrop::new(HashMap::new());
        r
    }
}

pub struct SerializerCommons {
    shared: Arc<Mutex<SharedContext>>,
    permit: ManuallyDrop<MutexGuard<'static, SharedContext>>
}

impl SerializerCommons {
    pub async fn co_yield(&mut self) {
        self.release_permit();
        yield_now().await;
        self.acquire_permit().await;
    }

    pub async fn co_await<FUT, T>(&mut self, fut: FUT) -> T
        where FUT: Future<Output=T>,
              T: Send + Sync
    {
        self.release_permit();
        let ret: T = fut.await;
        self.acquire_permit().await;
        ret
    }

    pub async fn co_spawn<F, ARGS, FUT, T>(&mut self, f: F, args: ARGS) -> task::JoinHandle<T>
        where F: (FnOnce(&mut ChildSerializer, ARGS) -> FUT) + Send + 'static,
              ARGS: Send + 'static,
              FUT: Future<Output=T> + Send,
              T: Send + 'static
    {
        let task_id: u32 = self.permit.alloc_task_id();
        let mut permit: MutexGuard<'static, SharedContext> = unsafe {
            ManuallyDrop::take(&mut self.permit)
        };
        let (sender, receiver): (oneshot::Sender<()>, oneshot::Receiver<()>) = oneshot::channel();
        permit.running_tasks.insert(task_id, receiver);
        let child_serializer = ChildSerializer {
            commons: SerializerCommons {
                shared: self.shared.clone(),
                permit: ManuallyDrop::new(permit)
            },
            task_id
        };
        let x: task::JoinHandle<T> = task::spawn(async move {
            let mut child_serializer: ChildSerializer = child_serializer;
            let r: T = f(&mut child_serializer, args).await;
            unsafe { sender.send(()).unchecked_unwrap(); }
            r
        });
        self.acquire_permit().await;
        x
    }

    async fn acquire_permit(&mut self) {
        self.permit = ManuallyDrop::new(unsafe {
            transmute::<>(self.shared.lock().await)
        });
    }

    fn release_permit(&mut self) {
        unsafe {
            let _ = ManuallyDrop::take(&mut self.permit);
        }
    }
}

pub struct MainSerializer {
    commons: SerializerCommons
}

impl Drop for MainSerializer {
    fn drop(&mut self) {
        todo!()
    }
}

impl MainSerializer {
    pub async fn finish(&mut self) {
        todo!()
    }
}

pub struct ChildSerializer {
    commons: SerializerCommons,
    pub task_id: u32
}

impl Drop for ChildSerializer {
    fn drop(&mut self) {
        todo!()
    }
}
