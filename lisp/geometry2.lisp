(defpackage #:geometry2
  (:use #:common-lisp)
  (:export #:point #:make-point #:point-x #:point-y))

(in-package #:geometry2)

(defstruct point
  (x 0 :type (signed-byte 32))
  (y 0 :type (signed-byte 32)))

(defun test-translate ()
  (let* ((p (make-point :x 1 :y 2))
         (x (point-x p))
         (y (point-y p)))
    (declare (type point p)
             (type (signed-byte 32) x y))
    (incf x 3)
    (let ((y (+ y 4)))
      (declare (type (signed-byte 32) y))
      (assert (= 4 x))
      (assert (= 6 y)))))

(test-translate)
