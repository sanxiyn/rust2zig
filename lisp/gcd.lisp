(defpackage #:gcd
  (:use #:common-lisp)
  (:shadow #:gcd)
  (:export #:gcd))

(in-package #:gcd)

(declaim (ftype (function ((unsigned-byte 32) (unsigned-byte 32)) (unsigned-byte 32)) gcd))
(defun gcd (a b)
  (declare (type (unsigned-byte 32) a b))
  (loop while (/= b 0)
        do (let ((t2 b))
             (declare (type (unsigned-byte 32) t2))
             (setf b (rem a b))
             (setf a t2)))
  a)

(defun test-gcd ()
  (assert (= 2 (gcd 16 10))))

(test-gcd)
