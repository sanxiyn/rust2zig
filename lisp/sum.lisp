(defpackage #:sum
  (:use #:common-lisp)
  (:export #:sum #:sum2 #:sum-odd))

(in-package #:sum)

(declaim (ftype (function (vector) (signed-byte 32)) sum))
(defun sum (xs)
  (let ((total 0))
    (declare (type (signed-byte 32) total))
    (loop for x across xs
          do (incf total x))
    total))

(declaim (ftype (function (vector) (signed-byte 32)) sum2))
(defun sum2 (xs)
  (let ((total 0))
    (declare (type (signed-byte 32) total))
    (loop for i of-type fixnum from 0 below (length xs)
          do (incf total (aref xs i)))
    total))

(declaim (ftype (function (vector) (signed-byte 32)) sum-odd))
(defun sum-odd (xs)
  (let ((total 0))
    (declare (type (signed-byte 32) total))
    (loop for x across xs
          do (block continue
               (when (= (rem x 2) 0)
                 (return-from continue))
               (incf total x)))
    total))

(defun test-sum ()
  (let ((xs #(1 2 3 4 5))
        (total 0))
    (declare (type vector xs)
             (type (signed-byte 32) total))
    (loop for x across xs
          do (incf total x))
    (assert (= 15 total))
    (assert (= 15 (sum xs)))
    (setf total 0)
    (loop for x from 1 to 5
          do (incf total x))
    (assert (= 15 total))))

(defun test-sum2 ()
  (let ((xs #(1 2 3 4 5)))
    (declare (type vector xs))
    (assert (= 15 (sum2 xs)))))

(defun test-sum-odd ()
  (let ((xs #(1 2 3 4 5)))
    (declare (type vector xs))
    (assert (= 9 (sum-odd xs)))))

(test-sum)
(test-sum2)
(test-sum-odd)
