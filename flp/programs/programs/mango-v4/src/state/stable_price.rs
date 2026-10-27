use anchor_lang::prelude::*;
use derivative::Derivative;
use static_assertions::const_assert_eq;
use std::mem::size_of;

                                                         
   
                                                                       
                                                                      
                                                   
   
                                                           
                                                                                   
                                                                           
                                                                               
                                           
#[zero_copy]
#[derive(Derivative)]
#[derivative(Debug)]
pub struct StablePriceModel {
                                             
    pub stable_price: f64,

    pub last_update_timestamp: u64,

                                                   
                                                                                       
                                                                                                  
                                                                                                 
                            
    pub delay_prices: [f64; 24],

                                                                                          
                                          
    pub delay_accumulator_price: f64,

                                                          
    pub delay_accumulator_time: u32,

                                  
    pub delay_interval_seconds: u32,

                                                                                     
    pub delay_growth_limit: f32,

                                                                   
                                                                   
    pub stable_growth_limit: f32,

                                                                  
    pub last_delay_interval_index: u8,

                                                                                    
    pub reset_on_nonzero_price: u8,

    #[derivative(Debug = "ignore")]
    pub padding: [u8; 6],

    #[derivative(Debug = "ignore")]
    pub reserved: [u8; 48],
}
const_assert_eq!(
    size_of::<StablePriceModel>(),
    8 + 8 + 8 * 24 + 8 + 4 + 4 + 4 + 4 + 1 * 2 + 6 + 48
);
const_assert_eq!(size_of::<StablePriceModel>(), 288);
const_assert_eq!(size_of::<StablePriceModel>() % 8, 0);

impl Default for StablePriceModel {
    fn default() -> Self {
        Self {
            stable_price: 0.0,
            last_update_timestamp: 0,
            delay_prices: [0.0; 24],
            delay_accumulator_price: 0.0,
            delay_accumulator_time: 0,
            delay_interval_seconds: 60 * 60,                                
            delay_growth_limit: 0.06,                                    
            stable_growth_limit: 0.0003,                                                                                       
            last_delay_interval_index: 0,
            reset_on_nonzero_price: 0,
            padding: Default::default(),
            reserved: [0; 48],
        }
    }
}

impl StablePriceModel {
    pub fn reset_to_price(&mut self, oracle_price: f64, now_ts: u64) {
        self.stable_price = oracle_price;
        self.delay_prices = [oracle_price; 24];
        self.delay_accumulator_price = 0.0;
        self.delay_accumulator_time = 0;
        self.last_update_timestamp = now_ts;
        self.reset_on_nonzero_price = if oracle_price > 0.0 { 0 } else { 1 };
    }

    pub fn delay_interval_index(&self, timestamp: u64) -> u8 {
        ((timestamp / self.delay_interval_seconds as u64) % self.delay_prices.len() as u64) as u8
    }

    #[inline(always)]
    fn growth_clamped(target: f64, prev: f64, growth_limit: f64) -> f64 {
        let max = prev * (1.0 + growth_limit);
                                                                                       
                                                                                  
        let min = prev * (1.0 - growth_limit);
        target.clamp(min, max)
    }

    pub fn update(&mut self, now_ts: u64, oracle_price: f64) {
                                                                                                      
        if self.reset_on_nonzero_price == 1 && oracle_price > 0.0 {
            self.reset_to_price(oracle_price, now_ts);
        }

        let dt = now_ts.saturating_sub(self.last_update_timestamp);
                                                                                              
                                                        
                                                                                   
                            
        let min_dt = 10;
        let max_dt = 10 * 60;          
        if dt < min_dt {
            return;
        }
                                                  
        let full_delay_passed =
            dt > self.delay_prices.len() as u64 * self.delay_interval_seconds as u64;
        let dt_limited = dt.min(max_dt) as f64;
        self.last_update_timestamp = now_ts;

          
                             
          
        self.delay_accumulator_time += dt as u32;
        self.delay_accumulator_price += oracle_price * dt_limited;

        let delay_interval_index = self.delay_interval_index(now_ts);
        if delay_interval_index != self.last_delay_interval_index {
                                                                                        
                                               
            let new_delay_price = {
                                                    
                let prev = if self.last_delay_interval_index == 0 {
                    self.delay_prices[self.delay_prices.len() - 1]
                } else {
                    self.delay_prices[self.last_delay_interval_index as usize - 1]
                };
                let avg = self.delay_accumulator_price / (self.delay_accumulator_time as f64);
                Self::growth_clamped(avg, prev, self.delay_growth_limit as f64)
            };

                                                                          
            if full_delay_passed {
                self.delay_prices.fill(new_delay_price);
            } else if delay_interval_index > self.last_delay_interval_index {
                self.delay_prices
                    [self.last_delay_interval_index as usize..delay_interval_index as usize]
                    .fill(new_delay_price);
            } else {
                self.delay_prices[self.last_delay_interval_index as usize..].fill(new_delay_price);
                self.delay_prices[..delay_interval_index as usize].fill(new_delay_price);
            }

            self.delay_accumulator_price = 0.0;
            self.delay_accumulator_time = 0;
            self.last_delay_interval_index = delay_interval_index;
        }

        let delay_price = self.delay_prices[delay_interval_index as usize];

          
                              
          
        self.stable_price = {
            let prev_stable_price = self.stable_price;
            let fraction = if delay_price >= prev_stable_price {
                prev_stable_price / delay_price
            } else {
                delay_price / prev_stable_price
            };
            let growth_limit = (self.stable_growth_limit as f64) * fraction * fraction * dt_limited;
            Self::growth_clamped(oracle_price, prev_stable_price, growth_limit)
        };
    }
}

#[cfg(all(test, feature = "legacy-unit-tests"))]
mod tests {
    use super::*;

    fn run_and_print(
        model: &mut StablePriceModel,
        start: u64,
        dt: u64,
        steps: u64,
        price: fn(u64) -> f64,
    ) -> u64 {
        println!("step,timestamp,stable_price,delay_price");
        for i in 0..steps {
            let time = start + dt * (i + 1);
            model.update(time, price(time));
            println!(
                "{i},{time},{},{}",
                model.stable_price, model.delay_prices[model.last_delay_interval_index as usize]
            );
        }
        start + dt * steps
    }

    #[test]
    fn test_stable_price_10x() {
        let mut model = StablePriceModel::default();
        model.reset_to_price(1.0, 0);

        let mut t;
        t = run_and_print(&mut model, 0, 60, 60, |_| 10.0);
        assert!((model.stable_price - 1.8).abs() < 0.1);
        assert_eq!(&model.delay_prices[1..], [1.0; 23]);
        assert!((model.delay_prices[0] - 1.06).abs() < 0.01);
        assert_eq!(model.last_delay_interval_index, 1);
        assert_eq!(model.delay_accumulator_time, 0);
        assert_eq!(model.delay_accumulator_price, 0.0);

        t = run_and_print(&mut model, t, 10, 6 * 60, |_| 10.0);
        assert!((model.stable_price - 2.3).abs() < 0.1);
        assert_eq!(&model.delay_prices[2..], [1.0; 22]);
        assert!((model.delay_prices[0] - 1.06).abs() < 0.01);
        assert!((model.delay_prices[1] - 1.06 * 1.06).abs() < 0.01);
        assert_eq!(model.last_delay_interval_index, 2);
        assert_eq!(model.delay_accumulator_time, 0);
        assert_eq!(model.delay_accumulator_price, 0.0);

                                                               
        t = run_and_print(&mut model, t, 300, 12 * 23, |_| 10.0);
        assert!((model.stable_price - 7.4).abs() < 0.1);
        assert!(model.delay_prices[0] > model.delay_prices[23]);
        assert!(model.delay_prices[23] > model.delay_prices[22]);
        assert!(model.delay_prices[1] < model.delay_prices[0]);
        assert!(model.delay_prices[1] < model.delay_prices[2]);
        assert_eq!(model.last_delay_interval_index, 1);

        println!("{t}");
    }

    #[test]
    fn test_stable_price_characteristics_upwards() {
        let mut model = StablePriceModel::default();

        model.reset_to_price(1.0, 0);

        let mut last = 1;
        for i in 0..100000 {
            model.update(60 * (i + 1), 1000.0);
            let now = model.stable_price as i32;
            if now > last {
                last = now;
                println!("reached {now}x after {i} steps, {} hours", i as f64 / 60.0);
                if now == 10 {
                    break;
                }
            }
        }
    }

    #[test]
    fn test_stable_price_characteristics_downwards() {
        let mut model = StablePriceModel::default();
        let init = 10000.0;
        model.reset_to_price(init, 0);

        let mut last = 1;
        for i in 0..100000 {
            model.update(60 * (i + 1), 0.0);
            let now = (init / model.stable_price) as i32;
            if now > last {
                last = now;
                println!(
                    "reached 1/{now}x after {i} steps, {} hours",
                    i as f64 / 60.0
                );
                if now == 10 {
                    break;
                }
            }
        }
    }

    #[test]
    fn test_stable_price_average() {
        let mut model = StablePriceModel {
            delay_growth_limit: 10.00,
            ..StablePriceModel::default()
        };
        model.reset_to_price(1.0, 0);

        run_and_print(&mut model, 0, 60, 60, |t| if t > 1800 { 2.0 } else { 1.0 });
        println!("{}", model.delay_prices[0]);
        assert!((model.delay_prices[0] - 1.5).abs() < 0.01);
    }
}
