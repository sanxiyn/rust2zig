(defpackage #:bitset
  (:use #:common-lisp)
  (:export #:bit-set #:make-bit-set #:bit-set-with-capacity #:bit-set-contains #:bit-set-put #:bit-set-toggle))

(in-package #:bitset)

(defstruct bit-set
  (data 0 :type fixnum)
  (length 0 :type fixnum))

(declaim (ftype (function (fixnum) bit-set) bit-set-with-capacity))
(defun bit-set-with-capacity (bits)
  (make-bit-set :data 0 :length bits))

(declaim (ftype (function (bit-set fixnum) boolean) bit-set-contains))
(defun bit-set-contains (self bit)
  (and (< bit (bit-set-length self)) (logbitp bit (bit-set-data self))))

(declaim (ftype (function (bit-set fixnum) boolean) bit-set-put))
(defun bit-set-put (self bit)
  (assert (< bit (bit-set-length self)))
  (let ((prev (logbitp bit (bit-set-data self))))
    (declare (type boolean prev))
    (setf (bit-set-data self) (logior (bit-set-data self) (ash 1 bit)))
    prev))

(declaim (ftype (function (bit-set fixnum) null) bit-set-toggle))
(defun bit-set-toggle (self bit)
  (assert (< bit (bit-set-length self)))
  (setf (bit-set-data self) (logxor (bit-set-data self) (ash 1 bit)))
  nil)

(defun test-toggle ()
  (let ((b (bit-set-with-capacity 16)))
    (declare (type bit-set b))
    (bit-set-toggle b 1)
    (bit-set-put b 2)
    (bit-set-toggle b 2)
    (bit-set-put b 3)
    (assert (bit-set-contains b 1))
    (assert (not (bit-set-contains b 2)))
    (assert (bit-set-contains b 3))))

(test-toggle)
