(defpackage #:dot
  (:use #:common-lisp)
  (:export #:dot))

(in-package #:dot)

(declaim (ftype (function (vector vector) (signed-byte 32)) dot))
(defun dot (a b)
  (let ((sum 0))
    (declare (type (signed-byte 32) sum))
    (loop for x across a
          for y across b
          do (incf sum (* x y)))
    sum))

(defun test-dot ()
  (let ((a #(1 2 3))
        (b #(4 5 6)))
    (declare (type vector a b))
    (assert (= 32 (dot a b)))))

(test-dot)
