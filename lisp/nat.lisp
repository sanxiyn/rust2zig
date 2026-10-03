(defpackage #:nat
  (:use #:common-lisp)
  (:export #:nat #:nat-zero #:make-nat-zero #:nat-succ #:make-nat-succ #:nat-to-int))

(in-package #:nat)

(defstruct nat-zero)
(defstruct nat-succ
  (v0 nil :type nat))
(deftype nat () '(or nat-zero nat-succ))

(declaim (ftype (function () nat) nat-zero))
(defun nat-zero ()
  (make-nat-zero))

(declaim (ftype (function (nat) nat) nat-succ))
(defun nat-succ (n)
  (make-nat-succ :v0 n))

(declaim (ftype (function (nat) (signed-byte 32)) nat-to-int))
(defun nat-to-int (self)
  (etypecase self
    (nat-zero 0)
    (nat-succ (let ((n (nat-succ-v0 self)))
                (declare (type nat n))
                (+ 1 (nat-to-int n))))))

(defun test-to-int ()
  (let ((three (nat-succ (nat-succ (nat-succ (nat-zero))))))
    (declare (type nat three))
    (assert (= 3 (nat-to-int three)))))

(test-to-int)
