(defpackage #:div
  (:use #:common-lisp)
  (:export #:div #:div2))

(in-package #:div)

(declaim (ftype (function ((unsigned-byte 32) (unsigned-byte 32)) (or null (unsigned-byte 32))) div))
(defun div (a b)
  (if (= (rem a b) 0)
      (truncate a b)
      nil))

(declaim (ftype (function ((unsigned-byte 32) (unsigned-byte 32)) (unsigned-byte 32)) div2))
(defun div2 (a b)
  (let ((x (div a b)))
    (if x
        x
        0)))

(defun test-div ()
  (assert (equal 2 (div 6 3)))
  (assert (equal nil (div 7 3))))

(defun test-div2 ()
  (assert (= 2 (div2 6 3)))
  (assert (= 0 (div2 7 3))))

(test-div)
(test-div2)
