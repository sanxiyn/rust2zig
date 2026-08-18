(defpackage #:geometry
  (:use #:common-lisp)
  (:shadow #:min #:max)
  (:export #:point #:make-point #:point-x #:point-y #:point-translate #:shape #:shape-dot #:make-shape-dot #:shape-line #:make-shape-line #:shape-circle #:make-shape-circle #:bounding-box))

(in-package #:geometry)

(defstruct point
  (x 0 :type (signed-byte 32))
  (y 0 :type (signed-byte 32)))

(declaim (ftype (function (point (signed-byte 32) (signed-byte 32)) null) point-translate))
(defun point-translate (self dx dy)
  (incf (point-x self) dx)
  (incf (point-y self) dy)
  nil)

(defstruct shape-dot
  (v0 nil :type point))
(defstruct shape-line
  (v0 nil :type point)
  (v1 nil :type point))
(defstruct shape-circle
  (center nil :type point)
  (radius 0 :type (signed-byte 32)))
(deftype shape () '(or shape-dot shape-line shape-circle))

(declaim (ftype (function ((signed-byte 32) (signed-byte 32)) (signed-byte 32)) min))
(defun min (a b)
  (if (< a b)
      a
      b))

(declaim (ftype (function ((signed-byte 32) (signed-byte 32)) (signed-byte 32)) max))
(defun max (a b)
  (if (> a b)
      a
      b))

(declaim (ftype (function (shape) (values (signed-byte 32) (signed-byte 32) (signed-byte 32) (signed-byte 32))) bounding-box))
(defun bounding-box (s)
  (etypecase s
    (shape-dot (let ((p (shape-dot-v0 s)))
                 (declare (type point p))
                 (values (point-x p) (point-y p) (point-x p) (point-y p))))
    (shape-line (let ((p (shape-line-v0 s))
                      (q (shape-line-v1 s)))
                  (declare (type point p q))
                  (values (min (point-x p) (point-x q)) (min (point-y p) (point-y q)) (max (point-x p) (point-x q)) (max (point-y p) (point-y q)))))
    (shape-circle (let ((center (shape-circle-center s))
                        (radius (shape-circle-radius s)))
                    (declare (type point center)
                             (type (signed-byte 32) radius))
                    (values (- (point-x center) radius) (- (point-y center) radius) (+ (point-x center) radius) (+ (point-y center) radius))))))

(defun test-translate ()
  (let ((p (make-point :x 1 :y 2)))
    (declare (type point p))
    (point-translate p 3 4)
    (assert (= 4 (point-x p)))
    (assert (= 6 (point-y p)))))

(defun test-bounding-box-dot ()
  (let ((p (make-point :x 1 :y 2)))
    (declare (type point p))
    (multiple-value-bind (x0 y0 x1 y1) (bounding-box (make-shape-dot :v0 p))
      (declare (type (signed-byte 32) x0 y0 x1 y1))
      (assert (= 1 x0))
      (assert (= 2 y0))
      (assert (= 1 x1))
      (assert (= 2 y1)))))

(defun test-bounding-box-line ()
  (let ((p (make-point :x 1 :y 2))
        (q (make-point :x 2 :y 1)))
    (declare (type point p q))
    (multiple-value-bind (x0 y0 x1 y1) (bounding-box (make-shape-line :v0 p :v1 q))
      (declare (type (signed-byte 32) x0 y0 x1 y1))
      (assert (= 1 x0))
      (assert (= 1 y0))
      (assert (= 2 x1))
      (assert (= 2 y1)))))

(defun test-bounding-box-circle ()
  (let ((p (make-point :x 2 :y 2)))
    (declare (type point p))
    (multiple-value-bind (x0 y0 x1 y1) (bounding-box (make-shape-circle :center p :radius 1))
      (declare (type (signed-byte 32) x0 y0 x1 y1))
      (assert (= 1 x0))
      (assert (= 1 y0))
      (assert (= 3 x1))
      (assert (= 3 y1)))))

(test-translate)
(test-bounding-box-dot)
(test-bounding-box-line)
(test-bounding-box-circle)
