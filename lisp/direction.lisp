(defpackage #:direction
  (:use #:common-lisp)
  (:export #:direction #:opposite))

(in-package #:direction)

(deftype direction () '(member :north :east :south :west))

(declaim (ftype (function (direction) direction) opposite))
(defun opposite (d)
  (ecase d
    (:north :south)
    (:east :west)
    (:south :north)
    (:west :east)))

(defun test-direction ()
  (assert (equal :south (opposite :north)))
  (assert (equal :west (opposite :east)))
  (assert (equal :north (opposite :south)))
  (assert (equal :east (opposite :west))))

(test-direction)
