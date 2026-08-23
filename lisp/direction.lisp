(defpackage #:direction
  (:use #:common-lisp)
  (:export #:direction #:opposite #:vertical))

(in-package #:direction)

(deftype direction () '(member :north :east :south :west))

(declaim (ftype (function (direction) direction) opposite))
(defun opposite (d)
  (ecase d
    (:north :south)
    (:east :west)
    (:south :north)
    (:west :east)))

(declaim (ftype (function (direction) boolean) vertical))
(defun vertical (d)
  (ecase d
    ((:north :south) t)
    ((:east :west) nil)))

(defun test-opposite ()
  (assert (equal :south (opposite :north)))
  (assert (equal :west (opposite :east)))
  (assert (equal :north (opposite :south)))
  (assert (equal :east (opposite :west))))

(defun test-vertical ()
  (assert (equal t (vertical :north)))
  (assert (equal nil (vertical :east)))
  (assert (equal t (vertical :south)))
  (assert (equal nil (vertical :west))))

(test-opposite)
(test-vertical)
