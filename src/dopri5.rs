//===================================================================//
// Copyright (c) 2018, Sylvain Renevey
// Copyright (c) 2004, Ernst Hairer

// Redistribution and use in source and binary forms, with or without 
// modification, are permitted provided that the following conditions are 
// met:

// - Redistributions of source code must retain the above copyright 
// notice, this list of conditions and the following disclaimer.

// - Redistributions in binary form must reproduce the above copyright 
// notice, this list of conditions and the following disclaimer in the 
// documentation and/or other materials provided with the distribution.

// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS “AS 
// IS” AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED 
// TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A 
// PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE REGENTS OR 
// CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, 
// EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, 
// PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR 
// PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF 
// LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING 
// NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS 
// SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

// Written by:
//      Sylvain Renevey (syl.renevey@gmail.com)
//
// This code is a Rust adaptation of the code written originally
// in Fortran by:
//
//      E. Hairer & G. Wanner
//      Université de Genève, dept. de Mathématiques
//      CH-1211 Genève 4, Swizerland
//      E-mail : hairer@divsun.unige.ch, wanner@divsun.unige.ch
//
// and adapted for C by:
//      J.Colinge (colinge@divsun.unige.ch).
//
// and C++ by:
//      Blake Ashby (bmashby@stanford.edu)
//
//===================================================================//

use alga::linear::{InnerSpace, FiniteDimInnerSpace};
use alga::general::SubsetOf;
use std::f64;
use na;
use butcher_tableau::Dopri54;
use controller::Controller;

/// Enumeration of the types of the integration output.
#[derive(PartialEq)]
pub enum OutputType {
    Dense,
    Sparse,
}

/// Enumeration of the errors that may arise during integration.
pub enum IntegrationError {
    MaxNumStepReached,
    StepSizeUnderflow,
}

/// Contains some statistics of the integration.
#[derive(Clone,Copy)]
pub struct Stats {
    num_eval:           u32,
    accepted_steps:     u32,
    rejected_steps:     u32,
}

impl Stats {
    fn new() -> Stats {
        Stats {
            num_eval: 0,
            accepted_steps: 0,
            rejected_steps: 0
        }
    }

    /// Prints some statistics related to the integration process.
    pub fn print(&self) {
        println!("Number of function evaluations: {:?}", self.num_eval);
        println!("Number of accepted steps: {:?}", self.accepted_steps);
        println!("Number of rejected steps: {:?}", self.rejected_steps);
    }
}

/// Explicit Runge-Kutta method with Dormand-Prince coefficients of order 5(4) and dense output of order 4.
pub struct Dopri5<V> 
    where V: FiniteDimInnerSpace + Copy,
{
    f:              fn(f64, &V) -> V,
	x:              f64,
    x_old:          f64,
    x_end:          f64,
    xd:             f64,
    dx:             f64,
	y: 	            V,
	rtol:	        f64,
	atol:	        f64,
    x_out:          Vec<f64>,
    y_out:          Vec<V>,
    uround:         f64,
    h:              f64,
    h_old:          f64,
    n_max:          u32,
    coeffs:         Dopri54,
    controller:     Controller,
    out_type:       OutputType,
    rcont:          [V; 5],
    stats:          Stats
}


impl<V> Dopri5<V> 
    where V: FiniteDimInnerSpace + Copy,
          <V as InnerSpace>::Real: SubsetOf<f64>
{
    /// Default initializer for the structure
    ///
    /// # Arguments
    ///
    /// * `f`       - Pointer to the function to integrate
    /// * `x`       - Initial value of the independent variable (usually time)
    /// * `x_end`   - Final value of the independent variable
    /// * `dx`      - Increment in the dense output. This argument has no effect if the output type is Sparse
    /// * `y`       - Initial value of the dependent variable(s)
    /// * `rtol`    - Relative tolerance used in the computation of the adaptive step size
    /// * `atol`    - Absolute tolerance used in the computation of the adaptive step size
    ///
    pub fn new(f: fn(f64, &V) -> V, x: f64, x_end: f64, dx: f64, y: V, rtol: f64, atol: f64) -> Dopri5<V> {
        let alpha = 0.2 - 0.04*0.75;
        Dopri5 {
            f,
            x,
            xd: x,
            dx,
            x_old: x,
            x_end,
            y,
            rtol,
            atol,
            x_out: Vec::<f64>::new(),
            y_out: Vec::<V>::new(),
            uround: f64::EPSILON,
            h: 0.0,
            h_old: 0.0,
            n_max: 100000,
            coeffs: Dopri54::new(),
            controller: Controller::new(alpha, 0.04, 10.0, 0.2, x_end-x, 0.9),
            out_type: OutputType::Dense,
            rcont: [V::zero(); 5],
            stats: Stats::new()
        }
    }

    /// Advanced initializer for the structure.
    ///
    /// # Arguments
    ///
    /// * `f`       - Pointer to the function to integrate
    /// * `x`       - Initial value of the independent variable (usually time)
    /// * `x_end`   - Final value of the independent variable
    /// * `dx`      - Increment in the dense output. This argument has no effect if the output type is Sparse
    /// * `y`       - Initial value of the dependent variable(s)
    /// * `rtol`    - Relative tolerance used in the computation of the adaptive step size
    /// * `atol`    - Absolute tolerance used in the computation of the adaptive step size
    /// * `safety_factor`   - Safety factor used in the computation of the adaptive step size
    /// * `beta`    - Value of the beta coefficient of the PI controller
    /// * `fac_min` - Minimum factor between two successive steps
    /// * `fac_max` - Maximum factor between two successive steps
    /// * `h_max`   - Maximum step size
    /// * `h`       - Initial value of the step size
    /// * `n_max`   - Maximum number of iterations
    /// * `out_type`    - Type of the output. Must be a variant of the OutputType enum
    ///  
    pub fn from_param(f: fn(f64, &V) -> V, x: f64, x_end: f64, dx: f64, y: V, rtol: f64, atol: f64, safety_factor: f64, beta: f64, fac_min: f64, fac_max: f64, h_max: f64, h: f64, n_max: u32, out_type: OutputType) -> Dopri5<V> {
        let alpha = 0.2 - beta*0.75;
        Dopri5 {
            f,
            x,
            xd: x,
            x_old: 0.0,
            x_end,
            dx,
            y,
            rtol,
            atol,
            x_out: Vec::<f64>::new(),
            y_out: Vec::<V>::new(),
            uround: f64::EPSILON,
            h,
            h_old: 0.0,
            n_max,
            coeffs: Dopri54::new(),
            controller: Controller::new(alpha, beta, fac_max, fac_min, h_max, safety_factor),
            out_type,
            rcont: [V::zero(); 5],
            stats: Stats::new()
        }
    }

    /// Compute the initial stepsize
    fn hinit(&self) -> f64 {
        let f0 = (self.f)(self.x, &self.y);

        // Compute the norm of y0 and f0
        let dim = na::dimension::<V>();
        let mut d0 = 0.0;
        let mut d1 = 0.0;
        for i in 0..dim {
            let y_i: f64 = na::convert(self.y[i]);
            let sci: f64 = self.atol + y_i.abs() * self.rtol;
            d0 += (y_i/sci)*(y_i/sci);
            let f0_i: f64 = na::convert(f0[i]);
            d1 += (f0_i/sci)*(f0_i/sci);
        }

        // Compute h0
        let mut h0 = if d0 < 1.0E-10 || d1 < 1.0E-10 {
            1.0E-6
        } else {
            0.01*(d0/d1).sqrt()
        };

        h0 = h0.min(self.controller.h_max());

        let y1 = self.y + f0*na::convert(h0);
        let f1 = (self.f)(self.x+h0, &y1);
        
        // Compute the norm of f1-f0 divided by h0
        let mut d2: f64 = 0.0;
        for i in 0..dim {
            let f0_i: f64 = na::convert(f0[i]);
            let f1_i: f64 = na::convert(f1[i]);
            let y_i: f64 = na::convert(self.y[i]);
            let sci: f64 = self.atol + y_i.abs() * self.rtol;
            d2 += ((f1_i-f0_i)/sci) * ((f1_i-f0_i)/sci);
        }
        d2 = d2.sqrt() / h0;

        let h1 = if d1.sqrt().max(d2) <= 1.0E-15 {
            (1.0E-6 as f64).max(h0.abs()*1.0E-3)
        } else {
            (0.01/(d1.sqrt().max(d2))).powf(1.0/5.0)
        };

        (100.0*h0.abs()).min(h1.min(self.controller.h_max()))
    }

    /// Core integration method.
    pub fn integrate(&mut self) -> Result<Stats, IntegrationError> {
        
        // Initilization
        self.x_old = self.x;
        let mut n_step = 0;
        let mut last = false;
        let mut h_new = 0.0;
        let dim = na::dimension::<V>();

        if self.h == 0.0 {
            self.h = self.hinit();
            self.stats.num_eval += 2;
        }
        self.h_old = self.h;

        let mut k: Vec<V> = vec![V::zero(); 7];
        k[0] = (self.f)(self.x , &self.y);
        self.stats.num_eval += 1;

        // Main loop
        while !last {
            
            // Check if step number is within allowed range
            if n_step > self.n_max {
                println!("Stopped at x = {}. Need more than {} steps.", self.x, n_step);
                self.h_old = self.h;
                return Err(IntegrationError::MaxNumStepReached);
            }

            // Check for step size underflow
            if 0.1*self.h.abs() <= self.uround*self.x.abs() {
                println!("Stopped at x = {}. Step size underflow.", self.x);
                self.h_old = self.h;
                return Err(IntegrationError::StepSizeUnderflow);
            }

            // Check if it's the last iteration
            if self.x + 1.01*self.h - self.x_end > 0.0 {
                self.h = self.x_end - self.x;
                last = true;
            }
            n_step += 1;

            // 6 Stages
            let mut y_next = V::zero();
            for s in 1..7 {
                y_next = self.y.clone();
                for j in 0..s {
                    y_next += k[j]*na::convert( self.h * self.coeffs.a(s+1,j+1) );
                } 
                k[s] = (self.f)(self.x + self.h*self.coeffs.c(s+1) , &y_next);
            }
            k[1] = k[6];
            self.stats.num_eval += 6;


            // Prepare dense output
            if self.out_type == OutputType::Dense {
                self.rcont[4] = (k[0]*na::convert(self.coeffs.d(1)) + k[2]*na::convert(self.coeffs.d(3)) + k[3]*na::convert(self.coeffs.d(4)) + k[4]*na::convert(self.coeffs.d(5)) + k[5]*na::convert(self.coeffs.d(6)) + k[1]*na::convert(self.coeffs.d(7)))*na::convert(self.h);
            }


            // Compute error estimate
            k[3] =  (k[0]*na::convert(self.coeffs.e(1)) + k[1]*na::convert(self.coeffs.e(2)) + k[2]*na::convert(self.coeffs.e(3)) + k[3]*na::convert(self.coeffs.e(4)) + k[4]*na::convert(self.coeffs.e(5)) + k[5]*na::convert(self.coeffs.e(6)) + k[1]*na::convert(self.coeffs.e(7))) * na::convert(self.h);


            // Compute error
            let mut err = 0.0;
            for i in 0..dim {
                let y_i: f64 = na::convert(self.y[i]);
                let y_next_i: f64 = na::convert(y_next[i]);
                let sc_i: f64 = self.atol + y_i.abs().max(y_next_i.abs()) * self.rtol;
                let err_est_i: f64 = na::convert(k[3][i]);
                err += (err_est_i /sc_i)*(err_est_i /sc_i);
            }
            err = (err / dim as f64).sqrt();


            // Step size control
            if self.controller.accept(&err, &self.h, &mut h_new) {
                self.stats.accepted_steps += 1;

                // Prepare dense output
                if self.out_type == OutputType::Dense {
                    let ydiff = y_next - self.y;
                    let bspl = k[0]*na::convert(self.h) - ydiff;
                    self.rcont[0] = self.y;
                    self.rcont[1] = ydiff;
                    self.rcont[2] = bspl;
                    self.rcont[3] = -k[1]*na::convert(self.h) + ydiff - bspl;
                }

                k[0] = k[1];
                self.y = y_next;
                self.x_old = self.x;
                self.x += self.h;
                self.h_old = self.h;

                self.solution_output(y_next);

                // Normal exit
                if last {
                    self.h_old = h_new;
                    return Ok(self.stats);
                }
            } else {
                if self.stats.accepted_steps >= 1 {
                    self.stats.rejected_steps += 1;
                }
            }     
            self.h = h_new;
        }
        Ok(self.stats)
    }

    fn solution_output(&mut self, y_next: V) {
        if self.out_type == OutputType::Dense {
            while self.xd <= self.x {
                if self.x_old <= self.xd && self.x >= self.xd {
                    let theta = (self.xd - self.x_old)/self.h_old;
                    let theta1 = 1.0 - theta;
                    self.x_out.push(self.xd);
                    self.y_out.push(self.rcont[0] + (self.rcont[1] + (self.rcont[2] + (self.rcont[3] + self.rcont[4]*na::convert(theta1))*na::convert(theta))*na::convert(theta1))*na::convert(theta));
                    self.xd += self.dx;
                }
            }
        } else {
            self.x_out.push(self.x);
            self.y_out.push(y_next);
        }
    }

    /// Getter for the independent variable's output.
    pub fn x_out(&self) -> &Vec<f64> {
        &self.x_out
    }

    /// Getter for the dependent variables' output.
    pub fn y_out(&self) -> &Vec<V> {
        &self.y_out
    }
}