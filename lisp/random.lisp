(defpackage #:random
  (:use #:common-lisp)
  (:export #:rand32 #:make-rand32 #:+rand32-default-inc+ #:+rand32-multiplier+ #:rand32-new #:rand32-new-inc #:rand32-rand-u32))

(in-package #:random)

(defstruct rand32
  (state 0 :type (unsigned-byte 64))
  (inc 0 :type (unsigned-byte 64)))

(defconstant +rand32-default-inc+ 1442695040888963407)
(defconstant +rand32-multiplier+ 6364136223846793005)

(declaim (ftype (function ((unsigned-byte 64)) rand32) rand32-new))
(defun rand32-new (seed)
  (rand32-new-inc seed +rand32-default-inc+))

(declaim (ftype (function ((unsigned-byte 64) (unsigned-byte 64)) rand32) rand32-new-inc))
(defun rand32-new-inc (seed increment)
  (let ((rng (make-rand32 :state 0 :inc (logior (ldb (byte 64 0) (ash increment 1)) 1))))
    (declare (type rand32 rng))
    (rand32-rand-u32 rng)
    (setf (rand32-state rng) (ldb (byte 64 0) (+ (rand32-state rng) seed)))
    (rand32-rand-u32 rng)
    rng))

(declaim (ftype (function (rand32) (unsigned-byte 32)) rand32-rand-u32))
(defun rand32-rand-u32 (self)
  (let ((oldstate (rand32-state self)))
    (declare (type (unsigned-byte 64) oldstate))
    (setf (rand32-state self) (ldb (byte 64 0) (+ (ldb (byte 64 0) (* oldstate +rand32-multiplier+)) (rand32-inc self))))
    (let ((xorshifted (ldb (byte 32 0) (ash (logxor (ash oldstate (- 18)) oldstate) (- 27))))
          (rot (ldb (byte 32 0) (ash oldstate (- 59)))))
      (declare (type (unsigned-byte 32) xorshifted rot))
      (logior (ash xorshifted (- rot)) (ldb (byte 32 0) (ash xorshifted (- 32 rot)))))))

(defun test-rand32 ()
  (let* ((seed 54321)
         (r1 (rand32-new seed)))
    (declare (type (unsigned-byte 64) seed)
             (type rand32 r1))
    (assert (= 2891073575 (rand32-rand-u32 r1)))))

(test-rand32)
