(defpackage #:option
  (:use #:common-lisp)
  (:export #:option #:option-some #:make-option-some #:option-none #:make-option-none #:option-and #:option-is-none #:option-is-some #:option-unwrap))

(in-package #:option)

(defstruct option-some
  (v0 nil :type t))
(defstruct option-none)
(deftype option () '(or option-some option-none))

(declaim (ftype (function (option option) option) option-and))
(defun option-and (self optb)
  (etypecase self
    (option-some optb)
    (option-none (make-option-none))))

(declaim (ftype (function (option) boolean) option-is-none))
(defun option-is-none (self)
  (etypecase self
    (option-some nil)
    (option-none t)))

(declaim (ftype (function (option) boolean) option-is-some))
(defun option-is-some (self)
  (etypecase self
    (option-some t)
    (option-none nil)))

(declaim (ftype (function (option) t) option-unwrap))
(defun option-unwrap (self)
  (etypecase self
    (option-some (let ((x (option-some-v0 self)))
                   (declare (type t x))
                   x))
    (option-none (error "called unwrap on None"))))

(defun test-option ()
  (let ((x (make-option-some :v0 42))
        (y (make-option-none)))
    (declare (type option x y))
    (assert (equal t (option-is-some x)))
    (assert (equal nil (option-is-some y)))
    (assert (= 42 (option-unwrap x)))
    (let ((z (make-option-some :v0 7)))
      (declare (type option z))
      (assert (equal t (option-is-none (option-and y z)))))))

(test-option)
