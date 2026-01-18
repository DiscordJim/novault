use std::{collections::VecDeque, io::{Write, stdout}, sync::{Arc, LazyLock, Mutex}};

use anyhow::Result;
use colorize::AnsiColor;
use crossterm::{ExecutableCommand, cursor::{MoveDown, MoveUp}, terminal::{Clear, ClearType}};



pub enum LogType {
    Info,
    Error,
    Warn
}

#[macro_export]
macro_rules! console_log {
    ($lt:ident,  $($rest:tt)*) => {
        $crate::printing::_print_log($crate::printing::LogType::$lt);
        println!($($rest)*);
    };
}

pub fn _print_log(lt: LogType, ) {
    match lt {
        LogType::Info => print!("{} ", "INFO".green().bold()),
        LogType::Error => print!("{} ", "ERROR".red().bold()),
        LogType::Warn => print!("{} ", "WARN".yellow().bold())
    }
}

#[derive(Clone)]
struct CompLine {
    index: usize,
    text: String,
    in_progress: bool,
    depth: usize,
    size: usize,
}

static CURRENT_COMP: LazyLock<Mutex<Option<Arc<Mutex<SteppedComputation>>>>> = LazyLock::new(|| Mutex::default());

struct SteppedComputation {
    comp_name: String,
    size: usize,
    index: usize,
    history: Vec<CompLine>,
    first: bool,
    new_lines: usize,
    depth: usize,
    finished: bool
}

// #[derive(Clone)]
// pub struct SteppedComputationHandle {
//     previous: Option<Arc<Mutex<SteppedComputation>>>,
//     inner: Arc<Mutex<SteppedComputation>>
// }

#[allow(private_interfaces)]
pub enum SteppedComputationHandle {
    Shell {
        previous: Arc<Mutex<SteppedComputation>>,
        anchor: usize, 
        depth: usize,
        size: usize,
        index: usize,
        finished: bool
    },
    Active(Arc<Mutex<SteppedComputation>>)
}

impl SteppedComputationHandle {
    pub fn start(name: &str, size: usize) -> Self {

        let mut storage = CURRENT_COMP.lock().unwrap();
        

        match &mut *storage {
            Some(inner) => {
                let mut params = inner.lock().unwrap();
                params.size += 1;
                params.depth += 1;
                params.index += 1;

                let idx = params.index;
                let dpth = params.depth;
                let sie = params.size;
                params.add_feed_with_depth(name.to_string(), idx, dpth - 1, sie);

                let anchor = params.history.len() - 1;

                let depth = params.depth;


                Self::Shell {
                    previous: inner.clone(),
                    anchor,
                    depth,
                    size,
                    index: 0,
                    finished: false
                }
            }
            None => {
                let inn_arc =Arc::new(Mutex::new(SteppedComputation::start(name, size, true)));
                // let mut handle = ;
                *storage = Some(inn_arc.clone());

                // handle
                Self::Active(inn_arc)
            }
        }

        // let mut handle = Self {
        //     previous: None,
        //     inner: Arc::new(Mutex::new(SteppedComputation::start(name, size, storage.is_empty())))
        // };

        // if storage.is_empty() {
        //     storage.push(handle.clone());
        // } else {
        //     if let Some(last_elem) = storage.last_mut() {
                
        //         handle.previous = Some(last_elem.inner.clone());

        //         let mut prev_handle = last_elem.inner.lock().unwrap();
        //         prev_handle.size += 1;
        //         prev_handle.add_feed(name.to_string());
        //     }
        //     // storage.push(handle.clone());
        // }

        // handle
    }
    pub fn start_next<F, O>(&mut self, name: &str, done: &str, functor: F) -> O
    where 
        F: FnOnce() -> O
    {
        match self {
            Self::Active(real) => real.lock().unwrap().start_next(name, done, functor),
            Self::Shell { previous, anchor, depth, size, index, finished } => {
                let mut lock = previous.lock().unwrap();
                *index += 1;
                lock.add_feed_with_depth(name.to_string(), *index, *depth, *size);
                let o = functor();
                lock.finish_last_feed(done);

                o
            }
        }


        
    }
    pub fn finish(&mut self) {
        match self {
            Self::Active(inner) => {
                let mut outer = CURRENT_COMP.lock().unwrap();
                inner.lock().unwrap().finish();
                *outer = None;

            },
            Self::Shell { previous, anchor, depth, size, index, finished } => {
                if *finished {
                    return;
                }
                let mut lock = previous.lock().unwrap();
                lock.history = lock.history[..*anchor + 1].to_vec();
                lock.depth -= 1;
                *finished = true;
            }
        }
        // ;
    }
}

const SHOW_LIM: usize = 3;



impl SteppedComputation {

    /// Starts the computation, rendering a display.
    pub fn start(name: &str, size: usize, print: bool) -> Self {
        // console_log!(Info, "{name}");
        let mut s = Self {
            comp_name: name.to_string(),
            size,
            index: 0,
            history: vec![],
            first: true,
            new_lines: 0,
            depth: 1,
            finished: false
        };
        if print {
            s.render_printout();
        }   
        
        s

    }

    /// Starts the next step of the computation.
    pub fn start_next<F, O>(&mut self, name: &str, done: &str, functor: F) -> O
    where 
        F: FnOnce() -> O
    {
        self.add_feed(name.to_string());
        let o = functor();
        self.finish_last_feed(done);
        o
    }

    fn add_feed(&mut self, feed: String) {
        self.index += 1;
        self.add_feed_with_depth(feed, self.index, 1, self.size);
    }
    fn add_feed_with_depth(&mut self , feed: String,  index: usize, depth: usize, size: usize) {
        
        self.history.push(CompLine {
            index,
            text: feed,
            in_progress: true,
            depth,
            size: size
        });
        self.render_printout();
    }
    fn finish_last_feed(&mut self, new_text: &str) {
        if let Some(inner) = self.history.last_mut() {
            inner.in_progress = false;
            inner.text = new_text.to_string();
        }
        self.render_printout();
    }

    pub fn finish(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;
        let mut stout = stdout().lock();
        stout.execute(MoveUp(self.capped_history() as u16 - 1)).unwrap();

        for _ in 0..self.capped_history() - 1 {
            stout.execute(Clear(ClearType::CurrentLine)).unwrap();
            stout.execute(MoveDown(1)).unwrap();
        }
        stout.execute(MoveUp(self.capped_history() as u16 - 1)).unwrap();

    }

    fn capped_history(&self) -> usize {
        self.history.len().min(SHOW_LIM) + 1
    }

    fn percent_complete(&self) -> usize {
        let mut major = (self.index as f32 / self.size as f32) * 100.0 ;

        if self.depth > 1 {

 
            // If we have a depth greater than one we need to figure out how many steps.
            // let mut weight = self.depth;
            // let mut weight = 1.0 / self.size as f32;
            let mut weight_chain = vec![ 1.0 / self.size as f32 ];

            for i in 2..self.depth + 1 {

                
                if let Some(lin) = self.history
                    .iter()
                    .filter(|f| f.depth == i)
                    .max_by_key(|f| f.index) {
                        weight_chain.push(1.0 / lin.size as f32);

                        
                        let result = vec![ lin.index as f32 ].iter().chain(weight_chain.iter()).fold(1., |a,b| a * b );
                        // println!("depth: {i}, Weights: {:?}, Indx: {}, size: {}, result: {result}", weight_chain, lin.index, lin.size);
                        
                        major += result * 100.0;
                    
                    }
            }
        }


        major as usize
    }
   
    fn render_printout(&mut self) {
       
        let complete = self.percent_complete();
        // if 1 + 1 == 2 {
        //     return;
        // }
        if self.history.is_empty() {
            if !self.first {    
                stdout().lock().execute(MoveUp(1)).unwrap();
            } else {
                self.new_lines += 1;
                self.first = false;
            }
            console_log!(Info, "{} ({}%)", self.comp_name, complete);
        } else {
            let mut stout = stdout().lock();

            let capped_history = self.capped_history();

            if self.new_lines < capped_history {
                for _ in 0..(capped_history - self.new_lines) {
                    println!();
                }
                self.new_lines = capped_history;
            }
            stout.execute(MoveUp(capped_history as u16)).unwrap();
            

            
            // Render the title of the computation.
            console_log!(Info, "{} ({}%)              ", self.comp_name, complete);

            // Now we print.
            let start = if self.history.len() < SHOW_LIM {
                0
            } else {
                self.history.len() - SHOW_LIM
            };
            for i in start..self.history.len() {
                let elem = &self.history[i];
                let step_str =format!("({:?}/{})", elem.index, elem.size);


                stout.execute(Clear(ClearType::CurrentLine)).unwrap();

                for _ in 0..2 * elem.depth {
                    print!(" ");
                }

              
                if i == self.history.len() - 1 {
                    let colored_step_str = if elem.in_progress {
                        step_str.clone().yellow()
                    } else {
                        step_str.clone().green()
                    };
                    
                    println!("{colored_step_str} {}", elem.text);

                } else {
                    // stout.execute(Clear(ClearType::CurrentLine)).unwrap();
                    println!("{} {}", step_str.clone().grey().faint(), elem.text.clone().grey().faint().clone());
                }
            }


        }


        // Now we render out our liens.

    }
}

impl Drop for SteppedComputationHandle {
    fn drop(&mut self) {
        self.finish();

    }
}