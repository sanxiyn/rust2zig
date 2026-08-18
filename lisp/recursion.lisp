(defpackage #:recursion
  (:use #:common-lisp)
  (:export #:fib #:is-even #:is-odd))

(in-package #:recursion)

(declaim (ftype (function ((unsigned-byte 32)) (unsigned-byte 32)) fib))
(defun fib (n)
  (when (< n 2)
    (return-from fib n))
  (+ (fib (- n 1)) (fib (- n 2))))

(declaim (ftype (function ((unsigned-byte 32)) boolean) is-even))
(defun is-even (n)
  (when (= n 0)
    (return-from is-even t))
  (is-odd (- n 1)))

(declaim (ftype (function ((unsigned-byte 32)) boolean) is-odd))
(defun is-odd (n)
  (when (= n 0)
    (return-from is-odd nil))
  (is-even (- n 1)))

(defun test-fib ()
  (assert (= 55 (fib 10))))

(defun test-parity ()
  (assert (equal t (is-even 10)))
  (assert (equal nil (is-odd 10))))

(test-fib)
(test-parity)
