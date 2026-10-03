(defpackage #:fib
  (:use #:common-lisp)
  (:export #:fib))

(in-package #:fib)

(declaim (ftype (function ((unsigned-byte 32)) (unsigned-byte 32)) fib))
(defun fib (n)
  (let ((a 1)
        (b 1))
    (declare (type (unsigned-byte 32) a b))
    (loop for nil from 2 to n
          do (setf (values a b) (values b (+ a b))))
    a))

(defun test-fib ()
  (assert (= 55 (fib 10))))

(test-fib)
